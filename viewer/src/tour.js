// Scripted tour (?tour): a fixed timeline drives the real UI (opening the
// palette by click, `/` and Ctrl+K, typing into it, clicking its rows and the
// sidebar's lists, Enter and Esc; so the palette, showOption, clearSelection,
// the ladder and the graph's camera run as for a user) and an overlay draws
// a cursor, click ripples, key caps, captions and an end card. Option paths,
// module names and priorities come from the loaded graph.
//
//   ?tour          plays live (requestAnimationFrame) with Pause/Play/Restart
//                  and Exit. With prefers-reduced-motion it waits for Play and
//                  jumps instead of moving. Any real click or key ends the tour.
//   ?tour&t=<ms>   renders the frame at t as a pure function of t, then sets
//                  <html data-tour-frame="<ms>"> (tools/render-tour.sh).
//
// A step's effect depends only on the UI state at its start time, and
// continuous steps (typing, cursor moves) only on the elapsed time, so live
// playback and a fresh replay up to t end in the same state. The graph's
// camera and fades run on the tour's clock (ui.now). In frame mode CSS
// keyframe animations (palette, ladder cards) are seeked to the tour time at
// which they started, and CSS transitions are off.

const TOUR_DURATION = 19400;
const TOUR_END_CARD = 18100;
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
  // Rows of the sidebar's Switched off list (an option's first definition).
  const offs = a.switchedOff.filter(({ oi, di }, k) => a.switchedOff.findIndex((x) => x.oi === oi) === k && drawn(opts[oi].definitions[di]));
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

// A query that lists the option among the palette's first rows: a preferred
// word, else a path segment, else the whole path.
function tourQuery(oi, preferred) {
  const path = model.options[oi].path;
  for (const q of [preferred, ...path.toLowerCase().split(".").reverse(), path]) {
    const rows = paletteSearch(model, q).flatMap((g) => g.items);
    const i = rows.findIndex((r) => r.kind === "option" && r.index === oi);
    if (i >= 0 && i < 4) return q;
  }
  return path;
}

// Steps: { at, dur, start(run), update(run, elapsed) }. Captions: [from, to, parts].
function tourTimeline(story) {
  const steps = [];
  const captions = [];
  const step = (at, dur, start, update) => steps.push({ at, dur, start, update });
  const caption = (from, to, parts) => captions.push([from, to, parts]);
  const move = (at, dur, target) => step(at, dur, (r) => r.moveTo(at, dur, target(r)));
  const click = (at, expect) => step(at, 0, (r) => r.click(at, expect()));
  const key = (at, k, label, opts) => step(at, 0, (r) => r.key(at, k, label, opts));
  // Types `text` into the open, empty palette; returns the end time.
  const type = (at, text) => {
    const keys = [];
    let t = 0;
    for (let i = 1; i <= text.length; i++) keys.push([(t += 52 + 16 * Math.sin(i * 2.4)), text.slice(0, i)]);
    step(at, t, null, (r, e) => {
      const done = keys.filter(([k]) => k <= e).pop();
      if (done) r.typeValue(at + done[0], done[1]);
    });
    return at + t;
  };

  const trigger = () => $("#search-trigger");
  const palRow = (oi) => () => {
    const el = palette.rowOf("option", oi);
    if (!el) throw new Error(`tour: ${model.options[oi].path} is not in the palette`);
    return el;
  };
  const rowPoint = (oi) => () => {
    const r = palRow(oi)().getBoundingClientRect();
    return { x: r.left + Math.min(170, r.width * 0.4), y: r.top + r.height / 2 };
  };
  const offRow = () => {
    const li = $(`#lists .side-list[data-list="off"] li[data-oi="${story.off}"]`);
    if (!li) throw new Error("tour: the switched-off option is not in the sidebar list");
    return li.querySelector("button");
  };
  // Just below a module's pill, near its label's start, so the cursor covers
  // neither label nor chip. The camera is read at the clock's current time.
  const nodePoint = (id) => () => {
    graph.step();
    const r = graph.nodeRect(id);
    if (!r) throw new Error("tour: a module of the story is not drawn");
    return { x: r.x + Math.min(54, r.w * 0.45), y: r.y + r.h + 4 };
  };
  const inDetail = (sel, fx = 0.3, fy = 0.5) => () => {
    const el = $(`#detail ${sel}`);
    if (!el) throw new Error(`tour: no ${sel} in the detail panel`);
    return centerOf(el, fx, fy);
  };
  const aside = () => centerOf($("#lists"), 0.5, 0.75); // clear of the palette

  // 0-2.8 s: the start state; the palette opens from the sidebar.
  caption(150, 2700, ["Why is this option set to that?"]);
  move(900, 650, () => centerOf(trigger(), 0.3, 0.5));
  click(1600, trigger);

  // 2.8-6.1 s: mkForce beats mkDefault.
  let t = type(1800, tourQuery(story.force, "firewall"));
  move(t + 100, 420, rowPoint(story.force));
  click(t + 600, palRow(story.force));
  caption(t + 720, 6050, story.captions.force);
  move(t + 1150, 550, inDetail(".card.s-win .reason", 0.25));
  move(t + 2350, 600, nodePoint(story.forceLose));

  // 6.1-9.7 s: switched off by mkIf, from the sidebar's list.
  key(6150, "Escape", "Esc");
  move(6300, 600, () => centerOf(offRow(), 0.3, 0.3));
  click(7000, offRow);
  caption(7100, 9700, story.captions.off);
  move(7600, 650, nodePoint(story.offModule));

  // 9.7-13.6 s: your value beats the option default: `/`, type, Enter.
  move(9400, 300, aside);
  key(9800, "/", "/");
  t = type(10000, tourQuery(story.local, "locale"));
  move(t + 80, 380, rowPoint(story.local));
  key(t + 560, "Enter", "↵ Enter", { target: "#palette-input" });
  caption(t + 660, t + 3260, story.captions.local);
  move(t + 1100, 600, inDetail(".card.is-default .mpill", 0.5, 0.55));

  // 13.6-18.1 s: lists merge: Ctrl+K, type, click.
  move(13300, 300, aside);
  key(13700, "k", "Ctrl K", { ctrlKey: true });
  t = type(13900, tourQuery(story.list, "packages"));
  move(t + 80, 360, rowPoint(story.list));
  click(t + 560, palRow(story.list));
  caption(t + 660, TOUR_END_CARD, story.captions.list);
  move(t + 1150, 550, inDetail(".merged-title", 0.4, 0.6));

  steps.sort((x, y) => x.at - y.at);
  return { steps, captions };
}

// UI state of one playback: cursor, clicks and keys for the overlay.
class TourRun {
  constructor(reduced) {
    this.reduced = reduced;
    const pane = $("#stage").getBoundingClientRect();
    const p = { x: pane.left + pane.width * 0.82, y: pane.top + pane.height * 0.78 };
    this.cursor = { from: p, to: p, at: 0, dur: 0 };
    this.clicks = [];
    this.keys = [];
    this.focusAt = 0;
    this.typedAt = -1e9;
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
    if (focusable) focusable.focus();
    else if (document.activeElement) document.activeElement.blur();
    el.click();
    if (document.activeElement === $("#palette-input")) this.focusAt = at;
    this.clicks.push({ at, ...p });
  }

  key(at, key, label, opts = {}) {
    const target = opts.target ? $(opts.target) : document;
    target.dispatchEvent(new KeyboardEvent("keydown", { key, ctrlKey: !!opts.ctrlKey, bubbles: true, cancelable: true }));
    if (document.activeElement === $("#palette-input")) this.focusAt = at;
    this.keys.push({ at, label });
  }

  typeValue(at, value) {
    if ($("#palette-input").value === value) return;
    palette.setQuery(value);
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
    const logo = $(".brand .logo").cloneNode(true);
    logo.setAttribute("width", "34");
    logo.setAttribute("height", "34");
    this.end = div(
      "tour-end",
      div(
        "tour-card",
        h("div", { class: "tour-brand" }, logo, "optgraph"),
        h("p", {}, "Who sets each NixOS option, at what priority, and why it won."),
        h("pre", {}, cmd[0], h("em", {}, "<host>"), cmd[1]),
        h("p", { class: "muted" }, "Live demo: ", h("b", {}, TOUR_DEMO_URL.replace(/^https:\/\/|\/$/g, ""))),
      ),
    );
    this.el = h("div", { id: "tour" }, this.end, this.caption, this.caret, this.keycap, ...this.ripples, this.cursor);
    this.el.querySelectorAll(".tour-cursor, .tour-ripple, .tour-key, .tour-caret").forEach((x) => x.setAttribute("aria-hidden", "true"));
    this.caption.setAttribute("role", "status");
    if (live) {
      this.button = h("button", { type: "button", class: "btn" }, "Play");
      this.exit = h("button", { type: "button", class: "btn" }, "Exit tour");
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
    const pane = $("#stage").getBoundingClientRect();
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

    // What is under the cursor looks hovered; a palette row becomes the
    // active one, as on a real mouse move.
    const under = document.elementFromPoint(c.x, c.y);
    const row = under && under.closest(".pal-row");
    if (row && palette.isOpen && Number(row.dataset.n) !== palette.active) palette.setActive(Number(row.dataset.n), false);
    const hover = under && under.closest(".side-list li, button.mpill, .search-trigger, .override-card");
    if (hover !== this.hover) {
      if (this.hover) this.hover.classList.remove("tour-hover");
      if (hover) hover.classList.add("tour-hover");
      this.hover = hover;
    }

    // Drawn caret (the real one blinks on its own clock).
    const s = $("#palette-input");
    const focused = palette.isOpen && document.activeElement === s && t < TOUR_END_CARD;
    const blinkOn = reduced || t - run.typedAt < 500 || Math.floor((t - run.focusAt) / 530) % 2 === 0;
    show(this.caret, focused && blinkOn);
    if (focused) {
      const cs = getComputedStyle(s);
      this.measure.font = `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
      const r = s.getBoundingClientRect();
      const x = r.left + parseFloat(cs.paddingLeft) + this.measure.measureText(s.value).width - s.scrollLeft;
      place(this.caret, x + 1, r.top + (r.height - 20) / 2);
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
      this.caption.style.maxWidth = `${pane.width - 32}px`;
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
    this.live = live;
    this.reduced = reduced;
    this.clock = 0;
    this.anims = new Map(); // frame mode: CSS animation -> tour time it started
    ui.now = () => this.clock;
    ui.frozen = !live;
    if (!live) document.body.classList.add("tour-frame");
    this.overlay = new TourOverlay(live);
    graph.reserve.top = 78; // room for the captions (up to two lines)
    this.reset();
  }

  // Start state: palette closed, nothing selected, lists at the top.
  reset() {
    this.clock = 0;
    palette.close();
    $("#palette-input").value = "";
    $("#warnings-panel").hidden = true;
    $("#lists").scrollTop = 0;
    if (document.activeElement) document.activeElement.blur();
    clearSelection();
    graph.fit(false);
    this.run = new TourRun(this.reduced);
    this.next = 0;
    this.active = [];
    this.t = 0;
    this.anims.clear();
  }

  // Frame mode: CSS animations are put where they are at tour time `at`
  // (new ones started at `at`), so steps and the overlay see the geometry a
  // live playback would.
  settle(at) {
    if (this.live) return;
    for (const a of document.getAnimations()) {
      if (!this.anims.has(a)) this.anims.set(a, at);
      a.pause();
      a.currentTime = Math.max(0, at - this.anims.get(a));
    }
  }

  advance(t) {
    this.clock = t;
    this.active = this.active.filter((s) => {
      const e = this.reduced ? s.dur : Math.min(s.dur, t - s.at);
      if (s.update) s.update(this.run, e);
      return e < s.dur;
    });
    this.settle(t);
  }

  seek(t) {
    if (t < this.t) this.reset();
    while (this.next < this.steps.length && this.steps[this.next].at <= t) {
      const s = this.steps[this.next++];
      this.advance(s.at);
      if (s.start) s.start(this.run);
      this.settle(s.at);
      if (s.dur) this.active.push(s);
    }
    this.advance(t);
    this.t = t;
    this.overlay.render(t, this.run, this.captions, this.reduced);
    this.settle(t);
    graph.requestDraw();
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
    // Back on the page's own clock: the current view and dimming stay, the
    // next fit uses the full height.
    ui.now = () => performance.now();
    graph.cam = null;
    graph.fadeAt = -1e9;
    graph.reserve.top = 0;
    events.forEach((ev) => document.removeEventListener(ev, takeover, true));
    const p = new URLSearchParams(location.search);
    p.delete("tour");
    p.delete("t");
    const q = p.toString().replace(/=(?=&|$)/g, "");
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
