//! The window's root view: state, keyboard, and the shell (sidebar | centre
//! | inspector, top bar over the last two), as the web viewer's #shell.

use std::collections::{HashMap, HashSet};
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
    /// Options outside any instance.
    Options,
    /// Options directly in this instance (`Model::instances.list`).
    Instance(usize),
}

/// A row of the sidebar's lists.
#[derive(Clone, Debug)]
pub struct SideItem {
    pub list: ListKind,
    pub option: usize,
    /// Why it is listed: "mkForce in hardening.nix beats mkDefault in ..."
    pub note: String,
}

/// A child of the sidebar's scroll list, in order.
#[derive(Clone, Debug)]
pub enum SideRow {
    /// A list's header, with its count.
    Head(ListKind, usize),
    /// An empty list's note.
    Empty(ListKind),
    /// `side_items[i]`.
    Item(usize),
    /// N more than the list shows.
    More(usize),
    /// An instance group's header; the depth indents it.
    Group(usize, usize),
    /// The header of the instance groups, with how many there are.
    GroupsHead(usize),
}

/// A group with at most this many options starts open, as does the list of
/// top-level options with at most OPTIONS_OPEN_MAX.
pub const GROUP_OPEN_MAX: usize = 12;
pub const OPTIONS_OPEN_MAX: usize = 30;

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
    pub side_rows: Vec<SideRow>,
    pub side_cursor: Option<usize>,
    /// The sidebar shows only this instance and those inside it.
    pub instance_filter: Option<usize>,
    /// Instance groups the user opened or closed.
    pub group_open: HashMap<usize, bool>,
    /// The list of top-level options, when the user opened or closed it.
    pub options_open: Option<bool>,
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
    /// --opt / --module / --instance to apply once an evaluation has loaded.
    pub after_load: (Option<String>, Option<usize>, Option<String>),
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
            side_rows: Vec::new(),
            side_cursor: None,
            instance_filter: None,
            group_open: HashMap::new(),
            options_open: None,
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
            after_load: (None, None, None),
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
        self.model = Some(Rc::new(model));
        self.instance_filter = None;
        self.group_open.clear();
        self.options_open = None;
        self.rebuild_side();
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
        if let Sel::Option(oi) = sel {
            self.reveal_option(oi);
        }
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

    /// Filter the sidebar to an instance (and those inside it), or clear it.
    pub fn set_instance_filter(&mut self, inst: Option<usize>, cx: &mut Context<Self>) {
        self.instance_filter = inst;
        if let (Some(i), Some(m)) = (inst, self.model.clone()) {
            for a in m.instances.chain(i) {
                self.group_open.insert(a, true);
            }
        }
        self.rebuild_side();
        self.sidebar_scroll.set_offset(gpui::point(px(0.), px(0.)));
        // The graph opens the instance's cluster and looks at it.
        self.graph_sel = Some(self.sel);
        cx.notify();
    }

    /// `--instance` after an evaluation has loaded.
    pub fn apply_instance_arg(&mut self, arg: Option<String>, cx: &mut Context<Self>) {
        let Some(q) = arg else { return };
        let found = self.model.as_ref().map(|m| m.instances.find(&q));
        match found {
            Some(Ok(i)) => self.set_instance_filter(Some(i), cx),
            Some(Err(paths)) if !paths.is_empty() => {
                self.load_error = Some(format!("--instance {q} is ambiguous: {}", paths.join(", ")));
            }
            _ => self.load_error = Some(format!("no instance {q} in this graph")),
        }
    }

    /// Is an instance's group open: the user's choice, else open when small.
    pub fn group_is_open(&self, model: &Model, inst: usize) -> bool {
        self.group_open
            .get(&inst)
            .copied()
            .unwrap_or(model.instances.list[inst].total_options <= GROUP_OPEN_MAX)
    }

    pub fn toggle_group(&mut self, inst: usize, cx: &mut Context<Self>) {
        let Some(m) = self.model.clone() else { return };
        let open = self.group_is_open(&m, inst);
        self.group_open.insert(inst, !open);
        self.rebuild_side();
        cx.notify();
    }

    pub fn toggle_options_list(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.model.clone() else { return };
        let open = self
            .options_open
            .unwrap_or(m.instances.top_level.len() <= OPTIONS_OPEN_MAX);
        self.options_open = Some(!open);
        self.rebuild_side();
        cx.notify();
    }

    /// The groups around a selected nested option open, so it can be seen.
    fn reveal_option(&mut self, oi: usize) {
        let Some(m) = self.model.clone() else { return };
        let Some(i) = m.instances.of_option[oi] else { return };
        let mut changed = false;
        for a in m.instances.chain(i) {
            if !self.group_is_open(&m, a) {
                self.group_open.insert(a, true);
                changed = true;
            }
        }
        if changed {
            self.rebuild_side();
        }
    }

    /// Recomputes the sidebar's rows: for the filter, the open groups and the
    /// model. The cursor stays on its row if that is still there.
    pub fn rebuild_side(&mut self) {
        let keep = self
            .side_cursor
            .and_then(|c| self.side_items.get(c))
            .map(|s| (s.list, s.option));
        let (items, rows) = match self.model.clone() {
            Some(m) => self.side_lists(&m),
            None => (Vec::new(), Vec::new()),
        };
        self.side_items = items;
        self.side_rows = rows;
        self.side_cursor = keep.and_then(|k| self.side_items.iter().position(|s| (s.list, s.option) == k));
    }

    fn side_lists(&self, model: &Model) -> (Vec<SideItem>, Vec<SideRow>) {
        let a = &model.analysis;
        let is = &model.instances;
        // With a filter: only options in the instance or those inside it.
        let wanted: Option<HashSet<usize>> = self.instance_filter.map(|i| {
            is.subtree(i)
                .into_iter()
                .flat_map(|k| is.list[k].options.clone())
                .collect()
        });
        let keeps = |oi: usize| wanted.as_ref().is_none_or(|w| w.contains(&oi));
        let mut items: Vec<SideItem> = Vec::new();
        let mut rows: Vec<SideRow> = Vec::new();
        let mut list =
            |kind: ListKind, count: usize, entries: Vec<(usize, String)>, items: &mut Vec<SideItem>, show: bool| {
                let total = entries.len();
                rows.push(SideRow::Head(kind, count));
                if !show {
                    return;
                }
                if total == 0 {
                    rows.push(SideRow::Empty(kind));
                }
                for (option, note) in entries.into_iter().take(SIDE_LIST_LIMIT) {
                    rows.push(SideRow::Item(items.len()));
                    items.push(SideItem {
                        list: kind,
                        option,
                        note,
                    });
                }
                if total > SIDE_LIST_LIMIT {
                    rows.push(SideRow::More(total - SIDE_LIST_LIMIT));
                }
            };
        let overrides: Vec<(usize, String)> = a
            .overrides
            .iter()
            .filter(|&&oi| keeps(oi))
            .map(|&oi| (oi, a.info[oi].beats.as_ref().map(|b| b.by_module()).unwrap_or_default()))
            .collect();
        list(ListKind::Overrides, overrides.len(), overrides, &mut items, true);
        // One row per option; the count stays that of definitions (the summary's).
        let off_defs: Vec<_> = a.switched_off.iter().filter(|r| keeps(r.option)).collect();
        let mut off: Vec<(usize, Vec<String>)> = Vec::new();
        for r in &off_defs {
            let who = model.def_who(&model.options()[r.option].definitions[r.definition]);
            match off.iter_mut().find(|(oi, _)| *oi == r.option) {
                Some((_, whos)) => {
                    if !whos.contains(&who) {
                        whos.push(who);
                    }
                }
                None => off.push((r.option, vec![who])),
            }
        }
        let off: Vec<(usize, String)> = off
            .into_iter()
            .map(|(oi, whos)| (oi, format!("mkIf false in {}", whos.join(", "))))
            .collect();
        list(ListKind::Off, off_defs.len(), off, &mut items, true);
        let errors: Vec<(usize, String)> = a
            .errors
            .iter()
            .filter(|&&oi| keeps(oi))
            .map(|&oi| (oi, model.options()[oi].error.clone().unwrap_or_default()))
            .collect();
        list(ListKind::Errors, errors.len(), errors, &mut items, true);
        if self.instance_filter.is_none() {
            let open = self.options_open.unwrap_or(is.top_level.len() <= OPTIONS_OPEN_MAX);
            list(
                ListKind::Options,
                is.top_level.len(),
                is.top_level
                    .iter()
                    .map(|&oi| (oi, model.options()[oi].path.clone()))
                    .collect(),
                &mut items,
                open,
            );
        }
        if !is.is_empty() {
            let starts: Vec<usize> = match self.instance_filter {
                Some(i) => vec![i],
                None => is.roots.clone(),
            };
            rows.push(SideRow::GroupsHead(match self.instance_filter {
                Some(i) => is.subtree(i).len(),
                None => is.list.len(),
            }));
            let mut stack: Vec<(usize, usize)> = starts.into_iter().rev().map(|i| (i, 0)).collect();
            while let Some((i, depth)) = stack.pop() {
                rows.push(SideRow::Group(i, depth));
                if self.group_is_open(model, i) {
                    let inst = &is.list[i];
                    for &oi in inst.options.iter().take(SIDE_LIST_LIMIT) {
                        rows.push(SideRow::Item(items.len()));
                        items.push(SideItem {
                            list: ListKind::Instance(i),
                            option: oi,
                            note: model.options()[oi].path.clone(),
                        });
                    }
                    if inst.options.len() > SIDE_LIST_LIMIT {
                        rows.push(SideRow::More(inst.options.len() - SIDE_LIST_LIMIT));
                    }
                    for &c in inst.children.iter().rev() {
                        stack.push((c, depth + 1));
                    }
                }
            }
        }
        (items, rows)
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
