//! Submodule instances (`options[].instance`, `modules[].scope`): derived data
//! for grouping nested options and modules, and the breadcrumb of an option
//! inside an instance (`home-manager.users` › `ilya` › `programs.git.enable`).

use std::collections::{BTreeMap, HashMap};

use crate::analysis::{instance_name, parent_path};
use crate::model::Graph;

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    /// `home-manager.users.ilya`
    pub path: String,
    /// `ilya`
    pub name: String,
    /// Path of the option whose value holds the instance (`home-manager.users`).
    /// It equals `path` for a single submodule.
    pub parent: String,
    /// That option's index, when it is listed.
    pub parent_option: Option<usize>,
    /// The instance whose option holds this one (a submodule inside a submodule).
    pub enclosing: Option<usize>,
    pub children: Vec<usize>,
    /// 0 for an instance at the top level.
    pub depth: usize,
    /// Options directly in this instance.
    pub options: Vec<usize>,
    /// Modules directly in this instance (`modules[].scope`).
    pub modules: Vec<usize>,
    /// Modules that wrote this instance's definitions (`modules[].owner`).
    pub owners: Vec<usize>,
    /// The counts below include the instances inside this one.
    pub total_options: usize,
    pub total_modules: usize,
    pub overrides: usize,
    pub switched_off: usize,
    pub errors: usize,
}

/// A breadcrumb segment and what clicking it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crumb {
    pub text: String,
    pub target: CrumbTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrumbTarget {
    /// An option holding an instance.
    Option(usize),
    Instance(usize),
    /// The option itself.
    Here,
}

#[derive(Debug, Clone, Default)]
pub struct Instances {
    /// Sorted by path.
    pub list: Vec<Instance>,
    /// Those at the top level.
    pub roots: Vec<usize>,
    /// Option index -> its instance.
    pub of_option: Vec<Option<usize>>,
    /// Module index -> its instance.
    pub of_module: Vec<Option<usize>>,
    /// Indices of the options outside any instance.
    pub top_level: Vec<usize>,
    by_path: HashMap<String, usize>,
}

/// `path` below `prefix`: `a.b.c` below `a` is `b.c`.
pub fn relative_to<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    path.strip_prefix(prefix)?.strip_prefix('.')
}

impl Instances {
    /// `overrides`, `off` and `errors` are option indices (an option can be
    /// in `off` once per definition: it counts once).
    pub fn new(graph: &Graph, overrides: &[usize], off: &[usize], errors: &[usize]) -> Instances {
        let mut names: BTreeMap<&str, Option<&str>> = BTreeMap::new();
        for o in &graph.options {
            if let Some(i) = &o.instance {
                let parent = names.entry(i).or_default();
                if parent.is_none() {
                    *parent = o.parent.as_deref();
                }
            }
        }
        for m in &graph.modules {
            if let Some(s) = &m.scope {
                names.entry(s).or_default();
            }
        }
        let by_path: HashMap<String, usize> = names.keys().enumerate().map(|(i, p)| (p.to_string(), i)).collect();
        let option_at: HashMap<&str, usize> = graph
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| (o.path.as_str(), i))
            .collect();
        let mut list: Vec<Instance> = names
            .iter()
            .map(|(&path, parent)| {
                // A nested module without a nested option has no `parent`: its
                // instance is a single submodule if an option has its path.
                let parent = parent
                    .map(str::to_string)
                    .or_else(|| option_at.contains_key(path).then(|| path.to_string()))
                    .unwrap_or_else(|| parent_path(path).to_string());
                Instance {
                    path: path.to_string(),
                    name: instance_name(path),
                    parent_option: option_at.get(parent.as_str()).copied(),
                    parent,
                    enclosing: None,
                    children: Vec::new(),
                    depth: 0,
                    options: Vec::new(),
                    modules: Vec::new(),
                    owners: Vec::new(),
                    total_options: 0,
                    total_modules: 0,
                    overrides: 0,
                    switched_off: 0,
                    errors: 0,
                }
            })
            .collect();

        let of_option: Vec<Option<usize>> = graph
            .options
            .iter()
            .map(|o| o.instance.as_ref().map(|i| by_path[i]))
            .collect();
        let of_module: Vec<Option<usize>> = graph
            .modules
            .iter()
            .map(|m| m.scope.as_ref().map(|s| by_path[s]))
            .collect();
        for (oi, i) in of_option.iter().enumerate() {
            if let Some(i) = i {
                list[*i].options.push(oi);
            }
        }
        let module_at: HashMap<&str, usize> = graph
            .modules
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.as_str(), i))
            .collect();
        for (mi, i) in of_module.iter().enumerate() {
            let Some(i) = i else { continue };
            list[*i].modules.push(mi);
            if let Some(&owner) = graph.modules[mi].owner.as_deref().and_then(|o| module_at.get(o))
                && !list[*i].owners.contains(&owner)
            {
                list[*i].owners.push(owner);
            }
        }

        // The enclosing instance: the one its parent option lives in, else the
        // longest instance path above it.
        for i in 0..list.len() {
            let via_option = list[i].parent_option.and_then(|p| of_option[p]);
            let above = || {
                let mut p = parent_path(&list[i].path);
                while !p.is_empty() {
                    if let Some(&e) = by_path.get(p) {
                        return Some(e);
                    }
                    p = parent_path(p);
                }
                None
            };
            let e = via_option.or_else(above).filter(|&e| e != i);
            list[i].enclosing = e;
        }
        let mut roots = Vec::new();
        for i in 0..list.len() {
            match list[i].enclosing {
                Some(e) => list[e].children.push(i),
                None => roots.push(i),
            }
        }
        for inst in list.iter_mut() {
            inst.total_options = inst.options.len();
            inst.total_modules = inst.modules.len();
        }
        let count = |set: &[usize]| {
            let mut seen = vec![false; of_option.len()];
            let mut per = vec![0usize; list.len()];
            for &oi in set {
                if !seen[oi] {
                    seen[oi] = true;
                    if let Some(i) = of_option[oi] {
                        per[i] += 1;
                    }
                }
            }
            per
        };
        let (ov, of, er) = (count(overrides), count(off), count(errors));
        for i in 0..list.len() {
            list[i].overrides = ov[i];
            list[i].switched_off = of[i];
            list[i].errors = er[i];
        }
        // Children are added up into their enclosing instances, deepest first.
        fn depth_of(list: &[Instance], i: usize) -> usize {
            let mut d = 0;
            let mut at = i;
            while let Some(e) = list[at].enclosing {
                d += 1;
                at = e;
                if d > list.len() {
                    break; // a malformed graph: no endless walk
                }
            }
            d
        }
        for i in 0..list.len() {
            list[i].depth = depth_of(&list, i);
        }
        let mut order: Vec<usize> = (0..list.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(list[i].depth));
        for i in order {
            if let Some(e) = list[i].enclosing {
                let c = list[i].clone();
                let up = &mut list[e];
                up.total_options += c.total_options;
                up.total_modules += c.total_modules;
                up.overrides += c.overrides;
                up.switched_off += c.switched_off;
                up.errors += c.errors;
            }
        }
        let top_level = of_option
            .iter()
            .enumerate()
            .filter(|(_, i)| i.is_none())
            .map(|(oi, _)| oi)
            .collect();
        Instances {
            list,
            roots,
            of_option,
            of_module,
            top_level,
            by_path,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn index(&self, path: &str) -> Option<usize> {
        self.by_path.get(path).copied()
    }

    /// How many options sit in an instance.
    pub fn nested_options(&self) -> usize {
        self.of_option.iter().filter(|i| i.is_some()).count()
    }

    /// An instance by its full path, or by its name when that is unique.
    /// `Err` lists the paths of an ambiguous name; it is empty for no match.
    pub fn find(&self, query: &str) -> Result<usize, Vec<String>> {
        if let Some(i) = self.index(query) {
            return Ok(i);
        }
        let hits: Vec<usize> = (0..self.list.len()).filter(|&i| self.list[i].name == query).collect();
        match hits.as_slice() {
            [one] => Ok(*one),
            _ => Err(hits.iter().map(|&i| self.list[i].path.clone()).collect()),
        }
    }

    /// `i` and the instances enclosing it, outermost first.
    pub fn chain(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        let mut at = i;
        while let Some(e) = self.list[at].enclosing {
            if out.contains(&e) {
                break;
            }
            out.push(e);
            at = e;
        }
        out.reverse();
        out
    }

    /// `i` and the instances inside it.
    pub fn subtree(&self, i: usize) -> Vec<usize> {
        let mut out = vec![i];
        let mut k = 0;
        while k < out.len() {
            for &c in &self.list[out[k]].children {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
            k += 1;
        }
        out
    }

    /// The instance's path below its enclosing instance (`files.todo` in
    /// `fixture.home.alice`), its full path at the top level.
    pub fn label(&self, i: usize) -> &str {
        let inst = &self.list[i];
        inst.enclosing
            .and_then(|e| relative_to(&inst.path, &self.list[e].path))
            .unwrap_or(&inst.path)
    }

    /// An option's path below its instance (`programs.git.enable`).
    pub fn relative_path<'a>(&self, oi: usize, path: &'a str) -> &'a str {
        self.of_option[oi]
            .and_then(|i| relative_to(path, &self.list[i].path))
            .unwrap_or(path)
    }

    /// The breadcrumb of an option in an instance, empty at the top level:
    /// `home-manager.users` › `ilya` › `programs.git.enable`; with a
    /// submodule inside a submodule `fixture.home` › `alice` › `files` ›
    /// `todo` › `text`; a single submodule has no name segment.
    pub fn breadcrumb(&self, oi: usize, path: &str) -> Vec<Crumb> {
        let Some(i) = self.of_option[oi] else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for at in self.chain(i) {
            let inst = &self.list[at];
            let holder = inst
                .enclosing
                .and_then(|e| relative_to(&inst.parent, &self.list[e].path))
                .unwrap_or(&inst.parent);
            out.push(Crumb {
                text: holder.to_string(),
                target: inst
                    .parent_option
                    .map_or(CrumbTarget::Instance(at), CrumbTarget::Option),
            });
            if inst.parent != inst.path {
                out.push(Crumb {
                    text: inst.name.clone(),
                    target: CrumbTarget::Instance(at),
                });
            }
        }
        out.push(Crumb {
            text: self.relative_path(oi, path).to_string(),
            target: CrumbTarget::Here,
        });
        out
    }
}
