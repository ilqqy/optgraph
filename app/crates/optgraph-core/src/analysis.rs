//! graph.json -> indexed model with the derived data of the web viewer
//! (viewer/src/data.js): overrides, switched-off definitions, errors, the
//! priority ladder of an option with a status and reason per definition.

use std::collections::{HashMap, HashSet};

use crate::instances::Instances;
use crate::model::{Condition, Definition, Graph, Module, OptionEntry, Origin};
use crate::text::{base_name, plural, prio_name};

/// Per-option facts for the lists and the start summary.
///   override: a listed definition lost: active, known priority, not a winner,
///     to a listed winner or to omitted nixpkgs winners (`nixpkgsWinners`).
///     The option default losing does not count (that is just setting an
///     option), and neither do omitted nixpkgs definitions that lose.
///   user_vs_user: winner and strongest loser both come from non-nixpkgs modules.
///   off: indices of mkIf-false definitions from non-nixpkgs modules.
#[derive(Debug, Clone, Default)]
pub struct OptionInfo {
    pub has_user: bool,
    pub override_: bool,
    pub user_vs_user: bool,
    pub beats: Option<Beats>,
    pub off: Vec<usize>,
}

/// "mkForce" in "hardening.nix" beats "mkDefault" in "dev-tools.nix".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Beats {
    pub win: String,
    pub win_who: String,
    pub win_origin: String,
    pub lose: String,
    pub lose_who: String,
    pub lose_origin: String,
}

impl Beats {
    /// "mkForce (user) beats mkDefault (input:shared)"
    pub fn by_origin(&self) -> String {
        format!(
            "{} ({}) beats {} ({})",
            self.win, self.win_origin, self.lose, self.lose_origin
        )
    }

    /// "mkForce in hardening.nix beats mkDefault in dev-tools.nix"
    pub fn by_module(&self) -> String {
        format!(
            "{} in {} beats {} in {}",
            self.win, self.win_who, self.lose, self.lose_who
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffRef {
    pub option: usize,
    pub definition: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub modules: usize,
    pub options: usize,
    /// Listed definitions, without option defaults.
    pub definitions: usize,
    pub overrides: usize,
    pub switched_off: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub info: Vec<OptionInfo>,
    /// User-vs-user overrides first, then by path.
    pub overrides: Vec<usize>,
    pub with_user: Vec<usize>,
    pub nixpkgs_only: Vec<usize>,
    pub switched_off: Vec<OffRef>,
    pub errors: Vec<usize>,
    pub stats: Stats,
}

/// A definition's place in the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Win,
    Lose,
    Off,
    Unknown,
}

impl Status {
    /// Badge text: the state is never told by colour alone.
    pub fn label(self) -> &'static str {
        match self {
            Status::Win => "Winner",
            Status::Lose => "Lost",
            Status::Off => "Switched off",
            Status::Unknown => "Unknown",
        }
    }

    /// Strength for "the best status of a module": win > lose > unknown > off.
    pub fn rank(self) -> u8 {
        match self {
            Status::Win => 3,
            Status::Lose => 2,
            Status::Unknown => 1,
            Status::Off => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderRow {
    /// Index into the option's `definitions`.
    pub def: usize,
    pub status: Status,
    /// "beaten by hardening.nix: 50 < 1000"
    pub reason: String,
}

/// What a module contributes to an option: its best definition status and
/// that definition's priority and condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleStatus {
    pub status: Status,
    pub priority: Option<i64>,
    pub condition: Option<Condition>,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub graph: Graph,
    pub mod_index: HashMap<String, usize>,
    /// Module index -> indices of the options it defines (listed definitions).
    pub mod_options: Vec<Vec<usize>>,
    /// Module index -> indices of the modules importing it.
    pub importers: Vec<Vec<usize>>,
    /// Lower-cased option paths, for search.
    pub lower_paths: Vec<String>,
    pub analysis: Analysis,
    /// Submodule instances: nested options and modules grouped.
    pub instances: Instances,
}

impl Model {
    pub fn new(graph: Graph) -> Model {
        let mod_index: HashMap<String, usize> = graph
            .modules
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.clone(), i))
            .collect();
        let mut mod_options = vec![Vec::new(); graph.modules.len()];
        for (oi, o) in graph.options.iter().enumerate() {
            let mut seen = HashSet::new();
            for d in &o.definitions {
                let Some(id) = &d.module else { continue };
                if !seen.insert(id) {
                    continue;
                }
                if let Some(&mi) = mod_index.get(id) {
                    mod_options[mi].push(oi);
                }
            }
        }
        let mut importers = vec![Vec::new(); graph.modules.len()];
        for (i, m) in graph.modules.iter().enumerate() {
            for t in &m.imports {
                if let Some(&ti) = mod_index.get(t) {
                    importers[ti].push(i);
                }
            }
        }
        let lower_paths = graph.options.iter().map(|o| o.path.to_lowercase()).collect();
        let mut model = Model {
            graph,
            mod_index,
            mod_options,
            importers,
            lower_paths,
            analysis: Analysis::default(),
            instances: Instances::default(),
        };
        model.analysis = model.analyse();
        let a = &model.analysis;
        let off: Vec<usize> = a.switched_off.iter().map(|r| r.option).collect();
        model.instances = Instances::new(&model.graph, &a.overrides, &off, &a.errors);
        model
    }

    pub fn modules(&self) -> &[Module] {
        &self.graph.modules
    }

    pub fn options(&self) -> &[OptionEntry] {
        &self.graph.options
    }

    pub fn module(&self, id: &str) -> Option<&Module> {
        self.mod_index.get(id).map(|&i| &self.graph.modules[i])
    }

    pub fn option_index(&self, path: &str) -> Option<usize> {
        self.graph
            .options
            .binary_search_by(|o| o.path.as_str().cmp(path))
            .ok()
            .or_else(|| {
                // Sorted bytewise by the producer; fall back to a scan for other input.
                self.graph.options.iter().position(|o| o.path == path)
            })
    }

    pub fn def_module(&self, d: &Definition) -> Option<&Module> {
        d.module.as_deref().and_then(|id| self.module(id))
    }

    /// A definition's file: its own, else its module's (docs/schema.md).
    pub fn def_file(&self, d: &Definition) -> Option<String> {
        if let Some(f) = &d.file {
            return Some(f.clone());
        }
        self.def_module(d).map(|m| m.file.clone())
    }

    /// Short name of who made a definition: the module's short label, or "the default".
    pub fn def_who(&self, d: &Definition) -> String {
        if d.is_default() {
            return "the default".into();
        }
        match self.def_module(d) {
            Some(m) => module_short_label(m),
            None => self.def_file(d).map(|f| base_name(&f)).unwrap_or_else(|| "?".into()),
        }
    }

    /// The defining module's origin; "declaration" for the option default.
    pub fn def_origin(&self, d: &Definition) -> String {
        match self.def_module(d) {
            Some(m) => m.origin.label(),
            None if d.is_default() => "declaration".into(),
            None => "unknown".into(),
        }
    }

    fn def_is_nixpkgs(&self, d: &Definition) -> bool {
        self.def_module(d).is_some_and(|m| m.origin == Origin::Nixpkgs)
    }

    fn analyse(&self) -> Analysis {
        let opts = &self.graph.options;
        let info: Vec<OptionInfo> = opts.iter().map(|o| self.option_info(o)).collect();
        let all = 0..opts.len();
        let mut overrides: Vec<usize> = all.clone().filter(|&i| info[i].override_).collect();
        // Stable: user-vs-user first, then path order.
        overrides.sort_by_key(|&i| !info[i].user_vs_user);
        let with_user = all
            .clone()
            .filter(|&i| !info[i].override_ && info[i].has_user)
            .collect();
        let nixpkgs_only = all
            .clone()
            .filter(|&i| !info[i].override_ && !info[i].has_user)
            .collect();
        let switched_off: Vec<OffRef> = all
            .clone()
            .flat_map(|i| {
                info[i].off.iter().map(move |&d| OffRef {
                    option: i,
                    definition: d,
                })
            })
            .collect();
        let errors: Vec<usize> = all.filter(|&i| opts[i].error.is_some()).collect();
        let definitions = opts
            .iter()
            .map(|o| o.definitions.iter().filter(|d| !d.is_default()).count())
            .sum();
        let stats = Stats {
            modules: self.graph.modules.len(),
            options: opts.len(),
            definitions,
            overrides: overrides.len(),
            switched_off: switched_off.len(),
            errors: errors.len(),
        };
        Analysis {
            info,
            overrides,
            with_user,
            nixpkgs_only,
            switched_off,
            errors,
            stats,
        }
    }

    fn option_info(&self, o: &OptionEntry) -> OptionInfo {
        let mut has_user = false;
        let mut loser: Option<&Definition> = None;
        let mut off = Vec::new();
        for (i, d) in o.definitions.iter().enumerate() {
            if d.is_default() {
                continue;
            }
            let user = !self.def_is_nixpkgs(d);
            has_user |= user;
            if o.is_winner(i) || d.shadowed_at.is_some() {
                continue;
            }
            if !d.active {
                if user && d.condition == Some(Condition::MkIfFalse) {
                    off.push(i);
                }
            } else if let Some(p) = d.priority
                && loser.is_none_or(|l| p < l.priority.unwrap_or(i64::MAX))
            {
                loser = Some(d);
            }
        }
        let hidden = o.hidden_winners();
        let winner = o.winners.first().map(|&w| &o.definitions[w]);
        // No known winner (none listed, none omitted): no override.
        let Some(loser) = loser.filter(|_| winner.is_some() || hidden > 0) else {
            return OptionInfo {
                has_user,
                off,
                ..Default::default()
            };
        };
        let wo = winner.map_or_else(|| "nixpkgs".into(), |w| self.def_origin(w));
        let lo = self.def_origin(loser);
        OptionInfo {
            has_user,
            override_: true,
            user_vs_user: wo != "nixpkgs" && lo != "nixpkgs",
            beats: Some(Beats {
                win: prio_name(o.highest_prio),
                win_who: winner.map_or_else(
                    || format!("{} ({hidden} not listed)", o.hidden_kind()),
                    |w| self.def_who(w),
                ),
                win_origin: wo,
                lose: prio_name(loser.priority),
                lose_who: self.def_who(loser),
                lose_origin: lo,
            }),
            off,
        }
    }

    /// The ladder of an option: its definitions sorted by priority (lowest
    /// first, unknown last, then listing order), each with a status and a
    /// one-line reason computed from the data:
    ///   win   "beats dev-tools.nix: 50 < 1000", "merged with 5 others at priority 100"
    ///   lose  "beaten by hardening.nix: 50 < 1000",
    ///         "beaten by 3 nixpkgs definitions at 100 (not listed): 100 < 1000"
    /// Omitted nixpkgs winners (`nixpkgsWinners`) count as winners here.
    ///   off   "switched off: its mkIf condition is false"
    ///   unknown (priority unknown, or no known winner)
    pub fn ladder(&self, oi: usize) -> Vec<LadderRow> {
        let o = &self.graph.options[oi];
        let hp = o.highest_prio;
        let hp_s = hp.map(|p| p.to_string()).unwrap_or_else(|| "null".into());
        let strongest_loser = o
            .definitions
            .iter()
            .enumerate()
            .filter(|(i, d)| d.active && d.priority.is_some() && !o.is_winner(*i) && d.shadowed_at.is_none())
            .fold(None::<&Definition>, |best, (_, d)| match best {
                Some(b) if b.priority <= d.priority => Some(b),
                _ => Some(d),
            });
        let first_winner = o.winners.first().map(|&w| &o.definitions[w]);
        let hidden = o.hidden_winners();
        let win_count = o.winner_count();
        let omitted_kind = o.hidden_kind();
        let not_listed = if hidden > 0 {
            format!(" ({hidden} {omitted_kind}, not listed)")
        } else {
            String::new()
        };
        let mut rows: Vec<LadderRow> = o
            .definitions
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let (status, reason) = if o.is_winner(i) {
                    let reason = if win_count > 1 {
                        format!(
                            "merged with {} at priority {hp_s}{not_listed}",
                            plural(win_count - 1, "other", None)
                        )
                    } else if let Some(l) = strongest_loser {
                        format!("beats {}: {hp_s} < {}", self.def_who(l), l.priority.unwrap_or_default())
                    } else if d.is_default() {
                        "nothing stronger is set: the default applies".into()
                    } else {
                        "the only active definition".into()
                    };
                    (Status::Win, reason)
                } else if let Some(at) = &d.shadowed_at {
                    (Status::Lose, self.dropped_at(at))
                } else if !d.active {
                    let reason = if d.condition == Some(Condition::MkIfError) {
                        "its mkIf condition threw or is not a bool"
                    } else {
                        "switched off: its mkIf condition is false"
                    };
                    (Status::Off, reason.into())
                } else if d.priority.is_none() {
                    (Status::Unknown, "reading its value threw: priority unknown".into())
                } else if hp.is_none() {
                    (Status::Unknown, "the winner is unknown (see the error)".into())
                } else if win_count == 0 {
                    // A graph.json from before omitted.nixpkgsWinners.
                    (Status::Unknown, "the winners are not listed".into())
                } else {
                    let p = d.priority.unwrap_or_default();
                    let reason = match first_winner {
                        None => format!(
                            "beaten by {} at {hp_s} (not listed): {hp_s} < {p}",
                            plural(hidden, &format!("{omitted_kind} definition"), None)
                        ),
                        Some(w) if win_count == 1 => format!("beaten by {}: {hp_s} < {p}", self.def_who(w)),
                        Some(_) => format!("beaten by {win_count} definitions at {hp_s}{not_listed}: {hp_s} < {p}"),
                    };
                    (Status::Lose, reason)
                };
                LadderRow { def: i, status, reason }
            })
            .collect();
        rows.sort_by_key(|r| (o.definitions[r.def].priority.unwrap_or(i64::MAX), r.def));
        rows
    }

    /// How many warnings of each `nested-*` code, by code.
    pub fn nested_warning_counts(&self) -> Vec<(String, usize)> {
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for w in &self.graph.meta.warnings {
            if w.code.starts_with("nested-") {
                *counts.entry(&w.code).or_default() += 1;
            }
        }
        counts.into_iter().map(|(c, n)| (c.to_string(), n)).collect()
    }

    /// Why a `shadowedAt` definition never reached its instance: "dropped at
    /// users.users (mkForce)". The priority that beat it is known only when
    /// it lost at a listed option; an instance's own definitions have none.
    fn dropped_at(&self, at: &str) -> String {
        match self.option_index(at).and_then(|p| self.graph.options[p].highest_prio) {
            Some(p) => format!("dropped at {at} ({})", prio_name(Some(p))),
            None => format!("dropped at {at}"),
        }
    }

    /// How many modules the winners come from (the option default counts as one).
    pub fn merged_modules(&self, oi: usize) -> usize {
        let o = &self.graph.options[oi];
        o.winners
            .iter()
            .map(|&w| o.definitions[w].module.as_deref())
            .collect::<HashSet<_>>()
            .len()
    }

    /// Module `mi`'s best status for option `oi` (win > lose > unknown > off).
    pub fn module_status(&self, mi: usize, oi: usize) -> Option<ModuleStatus> {
        let id = &self.graph.modules[mi].id;
        let o = &self.graph.options[oi];
        let mut best: Option<ModuleStatus> = None;
        for (i, d) in o.definitions.iter().enumerate() {
            if d.module.as_ref() != Some(id) {
                continue;
            }
            let status = if !d.active {
                Status::Off
            } else if o.is_winner(i) {
                Status::Win
            } else if o.highest_prio.is_none() {
                Status::Unknown
            } else {
                Status::Lose
            };
            if best.is_none_or(|b| status.rank() > b.status.rank()) {
                best = Some(ModuleStatus {
                    status,
                    priority: d.priority,
                    condition: d.condition,
                });
            }
        }
        best
    }

    /// The modules defining option `oi`, each with its best definition
    /// (win > lose > unknown > off), strongest first (status, then
    /// priority): the graph's rings. As the web viewer's graph (main.js
    /// focusOf), an active loser whose own priority is unknown is "unknown".
    pub fn defining_modules(&self, oi: usize) -> Vec<(usize, ModuleStatus)> {
        let o = &self.graph.options[oi];
        let mut order: Vec<usize> = Vec::new();
        let mut best: HashMap<usize, ModuleStatus> = HashMap::new();
        for (i, d) in o.definitions.iter().enumerate() {
            let Some(&mi) = d.module.as_deref().and_then(|id| self.mod_index.get(id)) else {
                continue;
            };
            let status = if !d.active {
                Status::Off
            } else if o.is_winner(i) {
                Status::Win
            } else if d.priority.is_none() || o.highest_prio.is_none() {
                Status::Unknown
            } else {
                Status::Lose
            };
            let s = ModuleStatus {
                status,
                priority: d.priority,
                condition: d.condition,
            };
            match best.get(&mi) {
                None => {
                    order.push(mi);
                    best.insert(mi, s);
                }
                Some(b) if status.rank() > b.status.rank() => {
                    best.insert(mi, s);
                }
                Some(_) => {}
            }
        }
        let mut out: Vec<(usize, ModuleStatus)> = order.into_iter().map(|mi| (mi, best[&mi])).collect();
        out.sort_by_key(|(_, s)| (std::cmp::Reverse(s.status.rank()), s.priority.unwrap_or(i64::MAX)));
        out
    }
}

/// Inline user modules carry nixpkgs' flake.nix as their file: label them by
/// their place in nixosSystem's `modules` list (and position) instead.
pub fn module_label(m: &Module) -> String {
    use crate::text::short_path;
    if let Some(scope) = &m.scope {
        return format!("{scope} › {}", short_path(&nested_where(m)));
    }
    if m.origin == Origin::UserInline {
        let place = m
            .position
            .as_deref()
            .map(|p| format!(" ({})", base_name(p)))
            .unwrap_or_default();
        return match m.modules_index {
            Some(i) => format!("inline module #{i}{place}"),
            None => format!("nested inline module{place}"),
        };
    }
    let anon = m.id != m.file;
    if anon {
        let p = m.position.as_deref().unwrap_or(&m.file);
        format!("{} (anonymous)", short_path(p))
    } else {
        short_path(&m.file)
    }
}

/// Short label for graph nodes: the file name, or the inline index.
pub fn module_short_label(m: &Module) -> String {
    if let Some(scope) = &m.scope {
        return format!("{} › {}", instance_name(scope), base_name(&nested_where(m)));
    }
    if m.origin == Origin::UserInline {
        return match m.modules_index {
            Some(i) => format!("inline #{i}"),
            None => "inline (nested)".into(),
        };
    }
    if m.id != m.file {
        format!("{} (anon)", base_name(m.position.as_deref().unwrap_or(&m.file)))
    } else {
        base_name(&m.file)
    }
}

/// The last component of a submodule instance path: `users.users.alice` ->
/// `alice`, `fileSystems."/boot"` -> `/boot`.
pub fn instance_name(scope: &str) -> String {
    if let Some(body) = scope.strip_suffix('"') {
        // The opening quote of the last component (skipping escaped quotes).
        let b = body.as_bytes();
        let mut i = b.len();
        while i > 0 {
            i -= 1;
            if b[i] == b'"' && (i == 0 || b[i - 1] != b'\\') {
                return body[i + 1..].to_string();
            }
        }
    }
    match scope.rfind('.') {
        Some(i) => scope[i + 1..].to_string(),
        None => scope.to_string(),
    }
}

/// The path above the last component: `users.users.alice` -> `users.users`,
/// `fileSystems."/boot"` -> `fileSystems`, `a` -> "".
pub fn parent_path(path: &str) -> &str {
    let end = match path.strip_suffix('"') {
        Some(body) => {
            let b = body.as_bytes();
            let mut i = b.len();
            loop {
                if i == 0 {
                    break 0;
                }
                i -= 1;
                if b[i] == b'"' && (i == 0 || b[i - 1] != b'\\') {
                    break i;
                }
            }
        }
        None => path.rfind('.').map_or(0, |i| i + 1),
    };
    path[..end].strip_suffix('.').unwrap_or("")
}

/// A nested module's own file name, without the instance: for labels inside a
/// cluster that already names the instance.
pub fn nested_file_label(m: &Module) -> String {
    base_name(&nested_where(m))
}

/// Where a module inside an instance lives: its file, or for inline user code
/// (whose file is nixpkgs' flake.nix) the file of its position.
fn nested_where(m: &Module) -> String {
    match (&m.origin, &m.position) {
        (Origin::UserInline, Some(p)) => match p.rsplit_once(':') {
            Some((f, line)) if line.bytes().all(|c| c.is_ascii_digit()) => f.to_string(),
            _ => p.clone(),
        },
        _ => m.file.clone(),
    }
}

/// "+N nixpkgs": the nixpkgs definitions counted instead of listed.
pub fn omitted_badge(o: &OptionEntry) -> Option<String> {
    let n = o.omitted_total();
    (n > 0).then(|| format!("+{} nixpkgs", crate::text::thousands(n)))
}

/// What the value block of a definition shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueView {
    /// A preview to show in mono (copyable).
    Preview(String),
    /// The option looks like a secret: no value was written.
    Redacted,
    /// No preview, with the reason.
    Missing(&'static str),
}

pub fn value_view(d: &Definition) -> ValueView {
    match d.value_preview.as_deref() {
        Some("<redacted>") => ValueView::Redacted,
        Some(v) => ValueView::Preview(v.to_string()),
        None if !d.active => ValueView::Missing("not evaluated: switched off"),
        None if d.priority.is_none() => ValueView::Missing("value threw (see the error)"),
        None => ValueView::Missing("no preview"),
    }
}

/// Chip text of a defining module in the graph: "✓ 50 mkForce",
/// "1000 mkDefault", "mkIf false".
pub fn ring_text(s: &ModuleStatus) -> String {
    use crate::text::priority_label;
    match s.status {
        Status::Off if s.condition == Some(Condition::MkIfError) => "mkIf error".into(),
        Status::Off => "mkIf false".into(),
        Status::Unknown if s.priority.is_none() => "? priority unknown".into(),
        Status::Unknown => format!("? {}", priority_label(s.priority)),
        Status::Win => format!("✓ {}", priority_label(s.priority)),
        Status::Lose => priority_label(s.priority),
    }
}
