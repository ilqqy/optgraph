// Scripted tour (?tour): a fixed timeline drives the real UI (typing into
// the search, clicking result rows and the start lists, Esc; so runSearch,
// showOption, clearSelection and the graph rings run as for a user) and an
// overlay draws a cursor, click ripples, key caps, captions and an end card.
// Option paths, module names and priorities come from the loaded graph.
//
//   ?tour          plays live (requestAnimationFrame) with Pause/Play/Restart
//                  and Exit. With prefers-reduced-motion it waits for Play and
//                  jumps instead of moving. Any real click or key ends the tour.
//   ?tour&t=<ms>   renders the frame at t as a pure function of t, then sets
//                  <html data-tour-frame="<ms>"> (tools/render-tour.sh).
//
// A step's effect depends only on the UI state at its start time, and
// continuous steps (typing, scrolling, cursor moves) only on the elapsed
// time, so live playback and a fresh replay up to t end in the same state.

const TOUR_DURATION = 16000;
const TOUR_END_CARD = 14500;
const TOUR_COMMAND = "nix run github:ilqqy/optgraph -- .#nixosConfigurations.<host> --html graph.html";
const TOUR_DEMO_URL = "https://ilqqy.github.io/optgraph/";

const clamp01 = (x) => Math.max(0, Math.min(1, x));
const easeInOut = (p) => (p < 0.5 ? 4 * p * p * p : 1 - (-2 * p + 2) ** 3 / 2);
const centerOf = (el, fx = 0.5, fy = 0.5) => {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width * fx, y: r.top + r.height * fy };
};

// The options the tour shows: the demo's, or on another graph the first that
// fits each scene. null if one is missing.
function tourStory() {
  const opts = model.options;
  const a = model.analysis;
  const all = opts.map((_, i) => i);
  const pick = (path, ok, pool = all) => {
    const i = opts.findIndex((o) => o.path === path);
    return i >= 0 && ok(i) ? i : pool.find(ok);
  };
  const drawn = (d) => d != null && d.module != null && graph.byId.has(d.module);
  const winner = (oi) => opts[oi].definitions[opts[oi].winners[0]];
  const strongestLoser = (oi) => {
    const w = new Set(opts[oi].winners);
    return opts[oi].definitions
      .filter((d, i) => d.kind === "definition" && d.active && d.priority != null && !w.has(i))
      .sort((x, y) => x.priority - y.priority)[0];
  };

  const force = pick(
    "networking.firewall.enable",
    (oi) => {
      const w = winner(oi);
      const l = strongestLoser(oi);
      return opts[oi].winners.length === 1 && w.priority === 50 && l && l.priority === 1000 && drawn(w) && drawn(l);
    },
    a.overrides,
  );
  const offs = a.switchedOff.filter(({ oi, di }) => drawn(opts[oi].definitions[di]));
  const off = offs.find(({ oi }) => opts[oi].path === "services.printing.enable") || offs[0];
  const local = pick("i18n.defaultLocale", (oi) => {
    const o = opts[oi];
    const w = winner(oi);
    const dflt = o.definitions.findIndex((d) => d.kind === "default");
    return o.winners.length === 1 && w.kind === "definition" && drawn(w) && model.modById.get(w.module).origin !== "nixpkgs" && dflt >= 0 && !o.winners.includes(dflt);
  });
  const list = pick("environment.systemPackages", (oi) => {
    const o = opts[oi];
    return o.winners.length > 1 && o.omitted.nixpkgsActive + o.omitted.nixpkgsInactive > 0 && o.winners.every((i) => drawn(o.definitions[i]));
  });
  if (force == null || !off || local == null || list == null) return null;

  const label = (d) => moduleShortLabel(model.modById.get(d.module));
  const w = winner(force);
  const l = strongestLoser(force);
  const listModules = new Set(opts[list].winners.map((i) => opts[list].definitions[i].module));
  return {
    force,
    forceWin: w.module,
    forceLose: l.module,
    off: off.oi,
    offModule: opts[off.oi].definitions[off.di].module,
    local,
    list,
    captions: {
      force: [[prioName(w.priority)], ` (${w.priority}) in `, [label(w)], " beats ", [prioName(l.priority)], ` (${l.priority}) in `, [label(l)]],
      off: ["Defined, but switched off by ", ["mkIf"]],
      local: ["Your value beats the nixpkgs default"],
      list: [`Lists merge: ${listModules.size} modules contribute`],
    },
  };
}

// A query that lists the option among the first rows: a preferred word, else
// a path segment, else the whole path.
function tourQuery(oi, preferred) {
  const path = model.options[oi].path;
  for (const q of [preferred, ...path.toLowerCase().split(".").reverse(), path]) {
    const i = searchOptions(model, q).findIndex((r) => r.index === oi);
    if (i >= 0 && i < 5) return q;
  }
  return path;
}

// Steps: { at, dur, start(run), update(run, elapsed) }. Captions: [from, to, parts].
function tourTimeline(story) {
  const steps = [];
  const captions = [];
  let query = "";
  const step = (at, dur, start, update) => steps.push({ at, dur, start, update });
  const caption = (from, to, parts) => captions.push([from, to, parts]);
  const move = (at, dur, target) => step(at, dur, (r) => r.moveTo(at, dur, target(r)));
  const click = (at, expect) => step(at, 0, (r) => r.click(at, expect()));
  // Erases the current query, then types `text`; returns the end time.
  const type = (at, text) => {
    const keys = [];
    let t = 0;
    for (let i = query.length; i > 0; i--) keys.push([(t += 22), query.slice(0, i - 1)]);
    for (let i = 1; i <= text.length; i++) keys.push([(t += 50 + 16 * Math.sin(i * 2.4)), text.slice(0, i)]);
    query = text;
    step(at, t, null, (r, e) => {
      const done = keys.filter(([k]) => k <= e).pop();
      if (done) r.typeValue(at + done[0], done[1]);
    });
    return at + t;
  };

  const search = () => $("#search");
  const searchPoint = () => centerOf(search(), 0.18, 0.55);
  const row = (oi) => {
    const i = results.findIndex((r) => r.index === oi);
    const el = [...$("#results").querySelectorAll(".row")].find((x) => x.style.top === `${i * resultList.rowHeight}px`);
    if (!el) throw new Error(`tour: ${model.options[oi].path} is not in the result list`);
    return el;
  };
  const rowPoint = (oi) => () => {
    const r = row(oi).getBoundingClientRect();
    return { x: r.left + Math.min(150, r.width * 0.45), y: r.top + r.height / 2 };
  };
  const offRow = () => {
    const head = [...$("#detail").querySelectorAll("h3")].find((x) => x.firstChild.textContent === "Switched off");
    const li = head && [...head.nextElementSibling.querySelectorAll("li")].find((x) => x.querySelector(".path").textContent === model.options[story.off].path);
    if (!li) throw new Error("tour: the switched-off option is not in the start list");
    return li;
  };
  // Just below the module's label (name and priority), so the cursor covers
  // neither. graph.draw() lays the labels out now, synchronously, so the spot
  // depends only on the UI state.
  const nodePoint = (id) => () => {
    graph.draw();
    const n = graph.byId.get(id);
    const c = graph.canvas.getBoundingClientRect();
    const b = (graph.labelBoxes || []).find((x) => x.n === n);
    if (b) return { x: c.left + b.x0 + Math.min(60, (b.x1 - b.x0) * 0.55), y: c.top + b.y1 + 3 };
    const R = graph.radius(n) * graph.t.k;
    return { x: c.left + graph.t.x + n.x * graph.t.k - R * 0.6, y: c.top + graph.t.y + n.y * graph.t.k + R + 5 };
  };

  // 0-2 s: start state.
  caption(150, 1900, ["Why is this option set to that?"]);
  move(850, 650, searchPoint);
  click(1550, search);

  // 2-6 s: mkForce beats mkDefault.
  let t = type(1700, tourQuery(story.force, "firewall"));
  move(t + 120, 420, rowPoint(story.force));
  click(t + 640, () => row(story.force));
  caption(t + 740, 5900, story.captions.force);
  move(t + 1000, 600, nodePoint(story.forceWin));
  move(t + 2300, 550, nodePoint(story.forceLose));

  // 6-9 s: switched off by mkIf, from the start panel's list.
  step(6000, 0, (r) => r.key(6000, "Escape", "Esc"));
  step(6150, 450, (r) => {
    const d = $("#detail");
    const li = offRow().getBoundingClientRect();
    const box = d.getBoundingClientRect();
    const from = d.scrollTop;
    const to = li.bottom <= box.bottom - 12 ? from : Math.min(d.scrollHeight - d.clientHeight, from + li.bottom - box.bottom + 60);
    r.scroll = { el: d, from, to };
  }, (r, e) => {
    const s = r.scroll;
    s.el.scrollTop = Math.round(s.from + (s.to - s.from) * easeInOut(e / 450));
  });
  move(6150, 600, (r) => {
    const p = centerOf(offRow(), 0.3);
    return { x: p.x, y: p.y - (r.scroll.to - r.scroll.el.scrollTop) }; // where the row ends up
  });
  click(6850, offRow);
  caption(6950, 8850, story.captions.off);
  move(7250, 600, nodePoint(story.offModule));

  // 9-12 s: your value beats the option default.
  move(8500, 400, searchPoint);
  click(8950, search);
  t = type(9050, tourQuery(story.local, "locale"));
  move(t + 100, 380, rowPoint(story.local));
  click(t + 560, () => row(story.local));
  caption(t + 660, 11750, story.captions.local);
  move(t + 900, 550, () => centerOf($("#detail .ladder li.winner pre"), 0.35, 0.5));

  // 12-14.5 s: lists merge.
  move(11600, 350, searchPoint);
  click(12000, search);
  t = type(12080, tourQuery(story.list, "packages"));
  move(t + 80, 330, rowPoint(story.list));
  click(t + 480, () => row(story.list));
  caption(t + 580, TOUR_END_CARD, story.captions.list);
  move(t + 750, 450, () => centerOf($("#detail .kv .badge"), 0.15, 1.15));

  steps.sort((x, y) => x.at - y.at);
  return { steps, captions };
}

// UI state of one playback: cursor, clicks and keys for the overlay.
class TourRun {
  constructor(reduced) {
    this.reduced = reduced;
    const pane = $("#graph-pane").getBoundingClientRect();
    const p = { x: pane.left + pane.width * 0.86, y: pane.top + pane.height * 0.8 };
    this.cursor = { from: p, to: p, at: 0, dur: 0 };
    this.clicks = [];
    this.keys = [];
    this.focusAt = 0;
    this.typedAt = -1e9;
    this.scroll = null;
  }

  cursorAt(t) {
    const c = this.cursor;
    const p = this.reduced || !c.dur ? 1 : easeInOut(clamp01((t - c.at) / c.dur));
    return { x: c.from.x + (c.to.x - c.from.x) * p, y: c.from.y + (c.to.y - c.from.y) * p };
  }

  moveTo(at, dur, to) {
    this.cursor = { from: this.cursorAt(at), to, at, dur };
  }

  // A click where the cursor is, on what is really there; it must land in
  // `expect`. Focus moves as for a mouse click.
  click(at, expect) {
    const p = this.cursorAt(at);
    const el = document.elementFromPoint(p.x, p.y);
    if (!el || !expect.contains(el)) {
      console.error(`tour: click at ${Math.round(p.x)},${Math.round(p.y)} missed its target`);
      return;
    }
    const focusable = el.closest("input, button, [tabindex]");
    if (focusable) {
      focusable.focus();
      this.focusAt = at;
    } else if (document.activeElement) document.activeElement.blur();
    el.click();
    this.clicks.push({ at, ...p });
  }

  key(at, key, label) {
    document.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    this.keys.push({ at, label });
  }

  typeValue(at, value) {
    const s = $("#search");
    if (s.value === value) return;
    s.value = value;
    runSearch();
    this.typedAt = at;
  }
}

class TourOverlay {
  constructor(live) {
    const div = (cls, ...kids) => h("div", { class: cls }, ...kids);
    this.cursor = div("tour-cursor");
    this.cursor.innerHTML =
      '<svg width="24" height="30" viewBox="0 0 24 30"><path d="M3 2v21.5l5.6-5.2 3.7 8.9 3.7-1.6-3.7-8.7h7.6z" fill="#fff" stroke="#0b0d10" stroke-width="1.6" stroke-linejoin="round"/></svg>';
    this.ripples = [div("tour-ripple"), div("tour-ripple")];
    this.keycap = div("tour-key");
    this.caret = div("tour-caret");
    this.caption = div("tour-caption");
    const cmd = TOUR_COMMAND.split("<host>");
    this.end = div(
      "tour-end",
      div(
        "tour-card",
        h("div", { class: "tour-brand" }, "optgraph"),
        h("p", {}, "Who sets each NixOS option, at what priority, and why it won."),
        h("pre", {}, cmd[0], h("em", {}, "<host>"), cmd[1]),
        h("p", { class: "muted" }, "Live demo: ", h("b", {}, TOUR_DEMO_URL.replace(/^https:\/\/|\/$/g, ""))),
      ),
    );
    this.el = h("div", { id: "tour" }, this.end, this.caption, this.caret, this.keycap, ...this.ripples, this.cursor);
    this.el.querySelectorAll(".tour-cursor, .tour-ripple, .tour-key, .tour-caret").forEach((x) => x.setAttribute("aria-hidden", "true"));
    this.caption.setAttribute("role", "status");
    if (live) {
      this.button = h("button", { type: "button" }, "Play");
      this.exit = h("button", { type: "button" }, "Exit tour");
      this.controls = h("div", { class: "tour-controls" }, this.button, this.exit);
      this.el.append(this.controls);
    }
    this.captionIndex = -1;
    this.hover = null;
    this.measure = document.createElement("canvas").getContext("2d");
    document.body.append(this.el);
    document.body.classList.add("touring");
  }

  remove() {
    this.el.remove();
    document.body.classList.remove("touring");
    if (this.hover) this.hover.classList.remove("tour-hover");
  }

  render(t, run, captions, reduced) {
    const show = (el, on) => (el.style.display = on ? "" : "none");
    const place = (el, x, y, extra = "") => (el.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)${extra}`);
    const fade = (from, to, ms = 220) => (reduced ? 1 : clamp01(Math.min((t - from) / ms, (to - t) / ms)));
    const pane = $("#graph-pane").getBoundingClientRect();
    const c = run.cursorAt(t);

    const pressed = run.clicks.some((k) => t >= k.at && t - k.at < 130);
    place(this.cursor, c.x - 3, c.y - 2, pressed ? " scale(0.85)" : "");

    const recent = reduced ? [] : run.clicks.filter((k) => t >= k.at && t - k.at < 550).slice(-2);
    this.ripples.forEach((el, i) => {
      const k = recent[i];
      show(el, k);
      if (!k) return;
      const p = (t - k.at) / 550;
      place(el, k.x - 22, k.y - 22, ` scale(${0.25 + 0.95 * easeInOut(p)})`);
      el.style.opacity = String(0.85 * (1 - p));
    });

    const key = run.keys.filter((k) => t >= k.at && t - k.at < 900).pop();
    show(this.keycap, key);
    if (key) {
      this.keycap.textContent = key.label;
      place(this.keycap, c.x + 20, c.y + 24);
      this.keycap.style.opacity = String(fade(key.at, key.at + 900, 150));
    }

    // Element under the cursor gets the hover look.
    const under = document.elementFromPoint(c.x, c.y);
    const hover = under && under.closest(".vlist .row, .picklist li, .linkish");
    if (hover !== this.hover) {
      if (this.hover) this.hover.classList.remove("tour-hover");
      if (hover) hover.classList.add("tour-hover");
      this.hover = hover;
    }

    // Drawn caret (the real one blinks on its own clock).
    const s = $("#search");
    const focused = document.activeElement === s && t < TOUR_END_CARD;
    const blinkOn = reduced || t - run.typedAt < 500 || Math.floor((t - run.focusAt) / 530) % 2 === 0;
    show(this.caret, focused && blinkOn);
    if (focused) {
      const cs = getComputedStyle(s);
      this.measure.font = `${cs.fontSize} ${cs.fontFamily}`;
      const r = s.getBoundingClientRect();
      const x = r.left + parseFloat(cs.borderLeftWidth) + parseFloat(cs.paddingLeft) + this.measure.measureText(s.value).width - s.scrollLeft;
      place(this.caret, x, r.top + (r.height - 17) / 2);
    }

    const ci = captions.findIndex(([from, to]) => t >= from && t < to);
    if (ci !== this.captionIndex) {
      this.captionIndex = ci;
      this.caption.replaceChildren(...(ci < 0 ? [] : captions[ci][2].map((p) => (typeof p === "string" ? p : h("code", {}, p[0])))));
    }
    show(this.caption, ci >= 0);
    if (ci >= 0) {
      const [from, to] = captions[ci];
      const a = fade(from, to);
      this.caption.style.maxWidth = `${pane.width - 40}px`;
      place(this.caption, pane.left + (pane.width - this.caption.offsetWidth) / 2, pane.top + 14 + (1 - a) * -8);
      this.caption.style.opacity = String(a);
    }

    const e = reduced ? (t >= TOUR_END_CARD ? 1 : 0) : clamp01((t - TOUR_END_CARD) / 300);
    show(this.end, e > 0);
    this.end.style.opacity = String(e);
    this.cursor.style.opacity = String(1 - e);

    if (this.controls) place(this.controls, pane.right - this.controls.offsetWidth - 12, pane.bottom - this.controls.offsetHeight - 12);
  }
}

class TourPlayer {
  constructor(story, live, reduced) {
    const { steps, captions } = tourTimeline(story);
    this.steps = steps;
    this.captions = captions;
    this.reduced = reduced;
    this.overlay = new TourOverlay(live);
    graph.reserveTop = 50; // room for the captions
    graph.fit();
    this.reset();
  }

  // Start state: nothing typed, nothing selected, lists at the top.
  reset() {
    const s = $("#search");
    s.value = "";
    s.blur();
    $("#warnings-panel").hidden = true;
    runSearch();
    clearSelection();
    this.run = new TourRun(this.reduced);
    this.next = 0;
    this.active = [];
    this.t = 0;
  }

  advance(t) {
    this.active = this.active.filter((s) => {
      const e = this.reduced ? s.dur : Math.min(s.dur, t - s.at);
      if (s.update) s.update(this.run, e);
      return e < s.dur;
    });
  }

  seek(t) {
    if (t < this.t) this.reset();
    while (this.next < this.steps.length && this.steps[this.next].at <= t) {
      const s = this.steps[this.next++];
      this.advance(s.at);
      if (s.start) s.start(this.run);
      if (s.dur) this.active.push(s);
    }
    this.advance(t);
    this.t = t;
    this.overlay.render(t, this.run, this.captions, this.reduced);
  }
}

function startTour() {
  const params = new URLSearchParams(location.search);
  const frame = params.get("t");
  const live = frame == null;
  const root = document.documentElement;
  root.dataset.tourDuration = String(TOUR_DURATION);
  const story = tourStory();
  if (!story) {
    const msg = "tour: this graph lacks the options the tour shows (it is made for the demo graph)";
    if (live) console.warn(msg);
    else root.dataset.tourError = msg;
    return;
  }

  if (!live) {
    const t = Math.max(0, Math.min(TOUR_DURATION, Number(frame) || 0));
    new TourPlayer(story, false, false).seek(t);
    // Rendered once the graph has drawn (it draws on animation frames).
    requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => (root.dataset.tourFrame = String(t)))));
    return;
  }

  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const player = new TourPlayer(story, true, reduced);
  const ov = player.overlay;
  let playing = false;
  let t0 = 0;
  const label = () => (ov.button.textContent = playing ? "Pause" : player.t >= TOUR_DURATION ? "Restart" : "Play");
  const tick = (now) => {
    if (!playing) return;
    const t = Math.min(TOUR_DURATION, now - t0);
    player.seek(t);
    if (t >= TOUR_DURATION) playing = false;
    else requestAnimationFrame(tick);
    label();
  };
  const play = () => {
    if (player.t >= TOUR_DURATION) player.seek(0);
    playing = true;
    t0 = performance.now() - player.t;
    requestAnimationFrame(tick);
    label();
  };
  // A real click, key or wheel outside the controls hands the page back.
  const takeover = (e) => {
    if (e.isTrusted && !ov.controls.contains(e.target)) exit();
  };
  const events = ["pointerdown", "keydown", "wheel"];
  const exit = () => {
    playing = false;
    ov.remove();
    graph.reserveTop = 0; // the current view stays; the next fit uses the full height
    events.forEach((ev) => document.removeEventListener(ev, takeover, true));
    const p = new URLSearchParams(location.search);
    p.delete("tour");
    p.delete("t");
    const q = p.toString();
    try {
      history.replaceState(null, "", location.pathname + (q ? `?${q}` : "") + location.hash);
    } catch (e) {
      // keep the old URL
    }
  };
  events.forEach((ev) => document.addEventListener(ev, takeover, true));
  ov.button.addEventListener("click", () => {
    if (playing) {
      playing = false;
      label();
    } else play();
  });
  ov.exit.addEventListener("click", exit);
  player.seek(0);
  if (reduced) label();
  else play();
}
