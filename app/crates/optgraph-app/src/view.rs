//! The window's root view: state, keyboard, and the shell (sidebar | centre
//! | inspector, top bar over the last two), as the web viewer's #shell.

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, ClipboardItem, Context, FocusHandle, Focusable, KeyBinding, Pixels, ScrollHandle, Window, actions, div,
    prelude::*, px,
};
use optgraph_core::Model;

use crate::graph::GraphState;
use crate::input::{InputEvent, TextInput};
use crate::palette::PaletteState;
use crate::theme::{FONT_UI, FS_MD, theme};

actions!(
    optgraph,
    [
        OpenPalette,
        OpenFile,
        Up,
        Down,
        Confirm,
        Cancel,
        FocusNext,
        FocusPrev,
        PalUp,
        PalDown,
        PalConfirm,
        PalClose
    ]
);

pub fn bindings() -> Vec<KeyBinding> {
    let shell = Some("Shell && !Palette && !TextInput");
    vec![
        KeyBinding::new("/", OpenPalette, shell),
        KeyBinding::new("ctrl-k", OpenPalette, Some("Shell")),
        KeyBinding::new("ctrl-o", OpenFile, Some("Shell")),
        KeyBinding::new("up", Up, shell),
        KeyBinding::new("down", Down, shell),
        KeyBinding::new("enter", Confirm, shell),
        KeyBinding::new("space", Confirm, Some("CompactRow")),
        KeyBinding::new("escape", Cancel, Some("Shell && !Palette")),
        KeyBinding::new("tab", FocusNext, Some("Shell")),
        KeyBinding::new("shift-tab", FocusPrev, Some("Shell")),
        KeyBinding::new("up", PalUp, Some("Palette")),
        KeyBinding::new("down", PalDown, Some("Palette")),
        KeyBinding::new("enter", PalConfirm, Some("Palette")),
        KeyBinding::new("escape", PalClose, Some("Palette")),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    None,
    Option(usize),
    Module(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Overrides,
    Off,
    Errors,
}

/// A row of the sidebar's lists.
#[derive(Clone, Debug)]
pub struct SideItem {
    pub list: ListKind,
    pub option: usize,
    /// Why it is listed: "mkForce in hardening.nix beats mkDefault in ..."
    pub note: String,
}

/// At most this many rows per sidebar list (as the web viewer).
pub const SIDE_LIST_LIMIT: usize = 200;

pub struct OptgraphView {
    pub model: Option<Rc<Model>>,
    pub source: String,
    pub load_error: Option<String>,
    pub sel: Sel,
    /// Compact (multi-winner) rows opened, by definition index.
    pub expanded: HashSet<usize>,
    /// The definition whose value was just copied (a check mark for a moment).
    pub copied: Option<usize>,
    pub path_copied: bool,
    pub focus: FocusHandle,
    /// Focus handles of the compact rows of the shown option, by ladder row.
    pub row_focus: Vec<FocusHandle>,
    /// The ladder row whose focus handle has the focus (keyboard ring).
    pub focused_row: Option<usize>,
    pub side_items: Vec<SideItem>,
    pub side_cursor: Option<usize>,
    pub warnings_open: bool,
    pub palette: PaletteState,
    pub inspector_scroll: ScrollHandle,
    pub sidebar_scroll: ScrollHandle,
    pub graph: GraphState,
    /// The selection the graph has not shown yet (it needs the window).
    pub graph_sel: Option<Sel>,
    pub bench: Option<crate::stage::Bench>,
    pub scheme: crate::loading::Scheme,
    /// The system explicitly prefers light (the desktop portal says so).
    pub system_light: bool,
    /// A running (or failed) `optgraph` evaluation of a flake reference.
    pub evaluation: Option<crate::evaluate::Evaluation>,
    /// --opt / --module to apply once an evaluation has loaded.
    pub after_load: (Option<String>, Option<usize>),
    _appearance: Option<gpui::Subscription>,
}

impl Focusable for OptgraphView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl OptgraphView {
    pub fn new(model: Option<Model>, source: String, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new("Search options and modules", cx));
        cx.subscribe(&input, |this: &mut Self, _, ev: &InputEvent, cx| match ev {
            InputEvent::Changed => this.palette_query_changed(cx),
        })
        .detach();
        let mut view = OptgraphView {
            model: None,
            source,
            load_error: None,
            sel: Sel::None,
            expanded: HashSet::new(),
            copied: None,
            path_copied: false,
            focus: cx.focus_handle(),
            row_focus: Vec::new(),
            focused_row: None,
            side_items: Vec::new(),
            side_cursor: None,
            warnings_open: false,
            palette: PaletteState::new(input),
            inspector_scroll: ScrollHandle::new(),
            sidebar_scroll: ScrollHandle::new(),
            graph: GraphState::default(),
            graph_sel: None,
            bench: crate::stage::Bench::from_env(),
            scheme: crate::loading::Scheme::System,
            system_light: false,
            evaluation: None,
            after_load: (None, None),
            _appearance: None,
        };
        if let Some(m) = model {
            view.set_model(m, cx);
        }
        view
    }

    /// Follow the system colour scheme from now on (unless pinned).
    pub fn watch_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._appearance =
            Some(cx.observe_window_appearance(window, |this, window, cx| this.system_scheme_changed(window, cx)));
        self.apply_scheme(window, cx);
        self.system_scheme_changed(window, cx);
    }

    pub fn set_model(&mut self, model: Model, cx: &mut Context<Self>) {
        let model = Rc::new(model);
        self.side_items = side_items(&model);
        self.model = Some(model);
        self.graph = GraphState::default();
        self.graph_sel = None;
        self.load_error = None;
        self.sel = Sel::None;
        self.side_cursor = None;
        self.expanded.clear();
        self.palette.open = false;
        cx.notify();
    }

    pub fn select(&mut self, sel: Sel, cx: &mut Context<Self>) {
        if self.sel != sel {
            self.expanded.clear();
            self.copied = None;
            self.path_copied = false;
        }
        self.sel = sel;
        self.graph_sel = Some(sel);
        let n = match (sel, &self.model) {
            (Sel::Option(oi), Some(m)) => m.options()[oi].definitions.len(),
            _ => 0,
        };
        self.row_focus = (0..n).map(|_| cx.focus_handle().tab_stop(true)).collect();
        self.inspector_scroll.set_offset(gpui::point(px(0.), px(0.)));
        if let Sel::Option(oi) = sel
            && let Some(i) = self.side_items.iter().position(|s| s.option == oi)
            && self.side_cursor.is_none_or(|c| self.side_items[c].option != oi)
        {
            self.side_cursor = Some(i);
        }
        cx.notify();
    }

    pub fn select_option_path(&mut self, path: &str, cx: &mut Context<Self>) -> bool {
        let Some(oi) = self.model.as_ref().and_then(|m| m.option_index(path)) else {
            return false;
        };
        self.select(Sel::Option(oi), cx);
        true
    }

    pub fn copy_value(&mut self, def: usize, text: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied = Some(def);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1400)).await;
            this.update(cx, |this, cx| {
                if this.copied == Some(def) {
                    this.copied = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn copy_path(&mut self, path: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(path));
        self.path_copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            this.update(cx, |this, cx| {
                this.path_copied = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_row(&mut self, def: usize, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.expanded.remove(&def) {
            self.expanded.insert(def);
        }
        if let Some(h) = self.row_focus.get(row) {
            window.focus(h, cx);
        }
        cx.notify();
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor(-1, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_cursor(1, cx);
    }

    /// Arrows walk the sidebar's lists and show each option.
    fn move_cursor(&mut self, d: isize, cx: &mut Context<Self>) {
        let n = self.side_items.len();
        if n == 0 {
            return;
        }
        let next = match self.side_cursor {
            None if d > 0 => 0,
            None => n - 1,
            Some(c) => (c as isize + d).clamp(0, n as isize - 1) as usize,
        };
        self.side_cursor = Some(next);
        let oi = self.side_items[next].option;
        self.select(Sel::Option(oi), cx);
        self.side_cursor = Some(next);
        self.sidebar_scroll.scroll_to_item(self.side_child_index(next));
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(c) = self.side_cursor {
            let oi = self.side_items[c].option;
            self.select(Sel::Option(oi), cx);
        }
    }

    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.warnings_open {
            self.warnings_open = false;
        } else {
            self.select(Sel::None, cx);
            self.side_cursor = None;
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn focus_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next(cx);
    }

    fn focus_prev(&mut self, _: &FocusPrev, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_prev(cx);
    }

    /// Width of the inspector: clamp(420px, 34vw, 540px).
    pub fn inspector_width(window: &Window) -> Pixels {
        let w = f32::from(window.viewport_size().width);
        px((w * 0.34).clamp(420., 540.))
    }
}

fn side_items(model: &Model) -> Vec<SideItem> {
    let a = &model.analysis;
    let mut out = Vec::new();
    for &oi in a.overrides.iter().take(SIDE_LIST_LIMIT) {
        let note = a.info[oi].beats.as_ref().map(|b| b.by_module()).unwrap_or_default();
        out.push(SideItem {
            list: ListKind::Overrides,
            option: oi,
            note,
        });
    }
    for r in a.switched_off.iter().take(SIDE_LIST_LIMIT) {
        let o = &model.options()[r.option];
        let who = model.def_who(&o.definitions[r.definition]);
        out.push(SideItem {
            list: ListKind::Off,
            option: r.option,
            note: format!("mkIf false in {who}"),
        });
    }
    for &oi in a.errors.iter().take(SIDE_LIST_LIMIT) {
        let note = model.options()[oi].error.clone().unwrap_or_default();
        out.push(SideItem {
            list: ListKind::Errors,
            option: oi,
            note,
        });
    }
    out
}

impl Render for OptgraphView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx).clone();
        let insp_w = Self::inspector_width(window);
        self.focused_row = self.row_focus.iter().position(|f| f.is_focused(window));
        let root = div()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_prev))
            .on_action(cx.listener(|this, _: &OpenPalette, window, cx| this.open_palette(window, cx)))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| this.prompt_open(window, cx)))
            .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| this.on_drop_paths(paths, window, cx)))
            .drag_over::<gpui::ExternalPaths>({
                let accent = t.accent;
                move |s, _, _, _| s.border_2().border_color(accent)
            })
            .relative()
            .size_full()
            .flex()
            .bg(t.bg)
            .text_color(t.text)
            .font_family(FONT_UI)
            .text_size(px(FS_MD))
            .line_height(gpui::relative(1.45));
        if let Some(ev) = self.evaluation.take() {
            let el = self.render_evaluation(&t, &ev);
            self.evaluation = Some(ev);
            return root.child(el).into_any_element();
        }
        let Some(model) = self.model.clone() else {
            return root.child(self.render_empty(&t, cx)).into_any_element();
        };
        root.child(self.render_sidebar(&t, &model, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.render_topbar(&t, &model, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(self.render_stage(&t, &model, insp_w, window, cx))
                            .child(self.render_inspector(&t, &model, insp_w, cx)),
                    ),
            )
            .when(self.warnings_open, |d| d.child(self.render_warnings(&t, &model, cx)))
            .when(self.palette.open, |d| d.child(self.render_palette(&t, &model, cx)))
            .into_any_element()
    }
}
