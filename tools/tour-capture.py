#!/usr/bin/env python3
"""tools/tour-capture.py SITE_DIR FRAMES_DIR [--fps N] [--width W] [--height H]

Serves SITE_DIR (the viewer's index.html and demo.json) on 127.0.0.1 and,
in one headless Chromium driven over the DevTools protocol, opens
?tour&t=<ms> for every frame of the viewer's tour and writes
FRAMES_DIR/NNNN.png. Dark colour scheme, device scale factor 1. Fails on any
console error, uncaught exception or browser log error.

Standard library only; CHROMIUM selects the browser (default: chromium).
The protocol runs over --remote-debugging-pipe (fds 3 and 4, NUL-delimited
JSON), so no WebSocket client is needed.
"""
import argparse
import base64
import fcntl
import functools
import http.server
import json
import math
import os
import shutil
import sys
import tempfile
import threading
import time

# Resolves once the page has rendered the frame (viewer/src/tour.js sets
# data-tour-frame, or data-tour-error).
READY = """new Promise((resolve, reject) => {
  const t0 = performance.now();
  (function check() {
    const d = document.documentElement.dataset;
    if (d.tourError) reject(new Error(d.tourError));
    else if (d.tourFrame === "%d") resolve(d.tourDuration);
    else if (performance.now() - t0 > 15000) reject(new Error("frame not rendered within 15 s"));
    else setTimeout(check, 5);
  })();
})"""


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path == "/favicon.ico":
            self.send_response(204)
            self.end_headers()
            return
        super().do_GET()


class Chromium:
    def __init__(self, exe, profile, log):
        to_r, to_w = os.pipe()
        from_r, from_w = os.pipe()
        # Duplicate the child's ends above fd 10 so the dup2 to 3 and 4 below
        # never has source == target (that would keep FD_CLOEXEC set).
        src_r = fcntl.fcntl(to_r, fcntl.F_DUPFD_CLOEXEC, 10)
        src_w = fcntl.fcntl(from_w, fcntl.F_DUPFD_CLOEXEC, 10)
        os.close(to_r)
        os.close(from_w)
        argv = [
            exe,
            "--headless",
            "--remote-debugging-pipe",
            f"--user-data-dir={profile}",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--disable-gpu",
            "--mute-audio",
            "about:blank",
        ]
        self.pid = os.posix_spawnp(
            exe,
            argv,
            os.environ,
            file_actions=[
                (os.POSIX_SPAWN_DUP2, src_r, 3),
                (os.POSIX_SPAWN_DUP2, src_w, 4),
                (os.POSIX_SPAWN_OPEN, 1, log, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644),
                (os.POSIX_SPAWN_DUP2, 1, 2),
            ],
        )
        os.close(src_r)
        os.close(src_w)
        self.wfd = to_w
        self.rfd = from_r
        self.chunks = []
        self.next_id = 0
        self.problems = []
        self.loaded = False

    def _message(self):
        while True:
            if self.chunks and b"\0" in self.chunks[-1]:
                data = b"".join(self.chunks)
                raw, _, rest = data.partition(b"\0")
                self.chunks = [rest] if rest else []
                return json.loads(raw)
            chunk = os.read(self.rfd, 1 << 20)
            if not chunk:
                raise RuntimeError("chromium closed the DevTools pipe")
            self.chunks.append(chunk)

    def _event(self, m):
        method = m.get("method")
        p = m.get("params", {})
        if method == "Page.loadEventFired":
            self.loaded = True
        elif method == "Runtime.exceptionThrown":
            d = p["exceptionDetails"]
            self.problems.append("exception: " + d.get("exception", {}).get("description", d.get("text", "?")))
        elif method == "Runtime.consoleAPICalled" and p["type"] in ("error", "assert", "warning"):
            text = " ".join(str(a.get("value", a.get("description", ""))) for a in p["args"])
            self.problems.append(f"console.{p['type']}: {text}")
        elif method == "Log.entryAdded" and p["entry"]["level"] == "error":
            e = p["entry"]
            self.problems.append(f"log ({e['source']}): {e['text']} {e.get('url', '')}".strip())

    def call(self, method, params=None, session=None):
        self.next_id += 1
        msg = {"id": self.next_id, "method": method, "params": params or {}}
        if session:
            msg["sessionId"] = session
        data = json.dumps(msg).encode() + b"\0"
        while data:
            data = data[os.write(self.wfd, data) :]
        while True:
            m = self._message()
            if m.get("id") == self.next_id:
                if "error" in m:
                    raise RuntimeError(f"{method}: {m['error'].get('message')}")
                return m.get("result", {})
            self._event(m)

    def wait_load(self):
        while not self.loaded:
            self._event(self._message())

    def close(self):
        try:
            self.call("Browser.close")
        except Exception:
            pass
        for _ in range(50):
            if os.waitpid(self.pid, os.WNOHANG)[0]:
                return
            time.sleep(0.1)
        os.kill(self.pid, 9)
        os.waitpid(self.pid, 0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("site")
    ap.add_argument("frames")
    ap.add_argument("--fps", type=int, default=15)
    ap.add_argument("--width", type=int, default=1280)
    ap.add_argument("--height", type=int, default=720)
    ap.add_argument("--only", help="comma-separated times in ms instead of the whole tour")
    a = ap.parse_args()

    exe = os.environ.get("CHROMIUM", "chromium")
    if not shutil.which(exe):
        sys.exit(f"tour-capture: {exe} not found (set CHROMIUM)")
    httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=a.site))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{httpd.server_address[1]}/"
    os.makedirs(a.frames, exist_ok=True)
    profile = tempfile.mkdtemp(prefix="tour-chromium-")
    browser = Chromium(exe, profile, os.path.join(a.frames, "chromium.log"))
    started = time.monotonic()
    try:
        target = browser.call("Target.createTarget", {"url": "about:blank"})["targetId"]
        s = browser.call("Target.attachToTarget", {"targetId": target, "flatten": True})["sessionId"]
        for m in ("Page.enable", "Runtime.enable", "Log.enable"):
            browser.call(m, session=s)
        browser.call(
            "Emulation.setDeviceMetricsOverride",
            {"width": a.width, "height": a.height, "deviceScaleFactor": 1, "mobile": False},
            s,
        )
        browser.call(
            "Emulation.setEmulatedMedia",
            {
                "features": [
                    {"name": "prefers-color-scheme", "value": "dark"},
                    {"name": "prefers-reduced-motion", "value": "no-preference"},
                ]
            },
            s,
        )
        browser.call("Emulation.setFocusEmulationEnabled", {"enabled": True}, s)
        browser.call("Emulation.setScrollbarsHidden", {"hidden": True}, s)

        def render(t, path):
            browser.loaded = False
            browser.call("Page.navigate", {"url": f"{base}?tour&t={t}"}, s)
            browser.wait_load()
            r = browser.call("Runtime.evaluate", {"expression": READY % t, "awaitPromise": True, "returnByValue": True}, s)
            if "exceptionDetails" in r:
                d = r["exceptionDetails"]
                browser.problems.insert(0, d.get("exception", {}).get("description", d.get("text", "?")))
            if browser.problems:
                raise RuntimeError(f"t={t}:\n  " + "\n  ".join(browser.problems))
            shot = browser.call("Page.captureScreenshot", {"format": "png"}, s)
            with open(path, "wb") as f:
                f.write(base64.b64decode(shot["data"]))
            return int(r["result"]["value"])

        if a.only:
            for t in (int(x) for x in a.only.split(",")):
                render(t, os.path.join(a.frames, f"t{t:05d}.png"))
            n = len(a.only.split(","))
        else:
            duration = render(0, os.path.join(a.frames, "0000.png"))
            n = math.ceil(duration * a.fps / 1000)
            for i in range(1, n):
                render(round(i * 1000 / a.fps), os.path.join(a.frames, f"{i:04d}.png"))
        print(f"tour-capture: {n} frames in {time.monotonic() - started:.1f} s, no console errors")
    finally:
        browser.close()
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as e:
        sys.exit(f"tour-capture: {e}")
