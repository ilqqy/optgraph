#!/usr/bin/env python3
"""tools/native-pointer.py WIDTH HEIGHT ACTIONS [--hold SECS]

A virtual pointer (wlr-virtual-pointer-unstable-v1) in the Wayland session
of $WAYLAND_DISPLAY, for tools/native-shot.sh: performs ACTIONS, then keeps
the pointer alive for --hold seconds (default 3) so hover states stay up
while the screenshot is taken. One device for the whole run: separate
wlrctl calls each add and remove a device, and clients drop the pointer
in between.

ACTIONS, separated by ';', positions in output pixels (WIDTH x HEIGHT):
  move X Y        jump there
  drag X Y        glide there in 20 steps (with the button down: a drag)
  down / up       left button
  click           down and up
  wheel DY        vertical wheel, DY > 0 scrolls down (zooms out the graph)
  sleep S

Needs pywayland and $OPTGRAPH_WAYLAND_XML / $OPTGRAPH_WLR_PROTOCOLS (the
native dev shell has them); the protocol bindings are generated at start.
"""
import os
import subprocess
import sys
import tempfile
import time

BTN_LEFT = 0x110


def bindings():
    root = tempfile.mkdtemp(prefix="optgraph-pointer-")
    # The generated modules import each other relatively: one package.
    out = os.path.join(root, "proto")
    os.makedirs(out)
    open(os.path.join(out, "__init__.py"), "w").close()
    xml = os.path.join(os.environ["OPTGRAPH_WLR_PROTOCOLS"], "unstable", "wlr-virtual-pointer-unstable-v1.xml")
    subprocess.run(
        [sys.executable, "-m", "pywayland.scanner", "-i", os.environ["OPTGRAPH_WAYLAND_XML"], xml, "-o", out],
        check=True,
    )
    sys.path.insert(0, root)


def main():
    args = sys.argv[1:]
    hold = 3.0
    if "--hold" in args:
        i = args.index("--hold")
        hold = float(args[i + 1])
        del args[i : i + 2]
    if len(args) != 3:
        sys.exit(__doc__)
    width, height, actions = int(args[0]), int(args[1]), args[2]
    bindings()
    from pywayland.client import Display
    from proto.wayland import WlSeat
    from proto.wlr_virtual_pointer_unstable_v1 import ZwlrVirtualPointerManagerV1

    display = Display()
    display.connect()
    registry = display.get_registry()
    found = {}

    def on_global(reg, name, interface, version):
        if interface == "zwlr_virtual_pointer_manager_v1":
            found["mgr"] = reg.bind(name, ZwlrVirtualPointerManagerV1, 1)
        elif interface == "wl_seat":
            found["seat"] = reg.bind(name, WlSeat, 1)

    registry.dispatcher["global"] = on_global
    display.roundtrip()
    if "mgr" not in found:
        sys.exit("native-pointer: the compositor has no virtual pointer manager")
    ptr = found["mgr"].create_virtual_pointer(found.get("seat"))
    display.roundtrip()
    pos = [width // 2, height // 2]
    t0 = time.monotonic()

    def ms():
        return int((time.monotonic() - t0) * 1000) & 0xFFFFFFFF

    def move(x, y):
        pos[:] = [x, y]
        ptr.motion_absolute(ms(), int(x), int(y), width, height)
        ptr.frame()
        display.flush()

    def button(state):
        ptr.button(ms(), BTN_LEFT, state)
        ptr.frame()
        display.flush()

    # Enter first, so the client has a pointer before anything happens.
    move(*pos)
    time.sleep(0.3)
    for action in filter(None, (a.strip() for a in actions.split(";"))):
        word, *rest = action.split()
        if word == "move":
            move(float(rest[0]), float(rest[1]))
        elif word == "drag":
            (x0, y0), (x1, y1) = pos, (float(rest[0]), float(rest[1]))
            for i in range(1, 21):
                move(x0 + (x1 - x0) * i / 20, y0 + (y1 - y0) * i / 20)
                time.sleep(0.016)
        elif word == "down":
            button(1)
        elif word == "up":
            button(0)
        elif word == "click":
            button(1)
            time.sleep(0.05)
            button(0)
        elif word == "wheel":
            # A wheel step as wheels send it: source, discrete step, value.
            dy = float(rest[0])
            ptr.axis_source(0)
            ptr.axis_discrete(ms(), 0, dy, 1 if dy > 0 else -1)
            ptr.frame()
            display.flush()
        elif word == "sleep":
            time.sleep(float(rest[0]))
        else:
            sys.exit(f"native-pointer: unknown action {word}")
        time.sleep(0.05)
    display.roundtrip()
    time.sleep(hold)
    ptr.destroy()
    display.roundtrip()
    display.disconnect()


if __name__ == "__main__":
    main()
