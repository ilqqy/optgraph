//! Fuzzy search for the command palette (viewer/src/search.js): substring
//! matches rank first, then subsequence matches with fewer gaps and
//! earlier/segment-start hits. Positions are char indices.

use std::collections::HashSet;

use crate::analysis::{Model, module_label, module_short_label};

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub score: f64,
    /// Char indices of the matched characters.
    pub hits: Vec<usize>,
}

/// `query` must be lower-case; `text` is the lower-cased `orig` (its
/// camelCase word starts count as segment starts).
pub fn fuzzy_match(query: &str, text: &str, orig: &str) -> Option<Match> {
    if query.is_empty() {
        return Some(Match {
            score: 0.,
            hits: vec![],
        });
    }
    let t: Vec<char> = text.chars().collect();
    let o: Vec<char> = orig.chars().collect();
    let q: Vec<char> = query.chars().collect();
    let o = if o.len() == t.len() { o } else { t.clone() };
    let word_start = |i: usize| i == 0 || matches!(t[i - 1], '.' | '/' | '-') || (o[i] != t[i] && o[i - 1] == t[i - 1]);
    let len = t.len() as f64;
    if let Some(sub) = find(&t, &q) {
        let hits = (sub..sub + q.len()).collect();
        let at_start = if sub == 0 || t[sub - 1] == '.' {
            50.
        } else if word_start(sub) {
            30.
        } else {
            0.
        };
        let at_end = if sub + q.len() == t.len() { 25. } else { 0. };
        return Some(Match {
            score: 1000. - sub as f64 - len * 0.1 + at_start + at_end,
            hits,
        });
    }
    let mut hits = Vec::with_capacity(q.len());
    let mut score = 0.;
    let mut last: Option<usize> = None;
    let mut ti = 0;
    for &c in &q {
        while ti < t.len() && t[ti] != c {
            ti += 1;
        }
        if ti == t.len() {
            return None;
        }
        if let Some(l) = last {
            score -= ((ti - l - 1).min(10)) as f64;
        }
        if word_start(ti) {
            score += 5.;
        }
        hits.push(ti);
        last = Some(ti);
        ti += 1;
    }
    Some(Match {
        score: score - len * 0.1,
        hits,
    })
}

fn find(t: &[char], q: &[char]) -> Option<usize> {
    if q.len() > t.len() {
        return None;
    }
    (0..=t.len() - q.len()).find(|&i| t[i..i + q.len()] == *q)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    Overrides,
    Off,
    Errors,
    Options,
    Modules,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::Overrides,
        Group::Off,
        Group::Errors,
        Group::Options,
        Group::Modules,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Group::Overrides => "Overrides",
            Group::Off => "Switched off",
            Group::Errors => "Errors",
            Group::Options => "Options",
            Group::Modules => "Modules",
        }
    }

    pub fn limit(self) -> usize {
        match self {
            Group::Options => 40,
            Group::Modules => 8,
            _ => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Option,
    Module,
}

/// Which text of a module matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    Label,
    Path,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub kind: Kind,
    pub index: usize,
    pub hits: Vec<usize>,
    pub place: Where,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupResult {
    pub group: Group,
    pub total: usize,
    pub items: Vec<Item>,
}

/// Palette groups for a query: options go to the first group they belong to
/// (Overrides, Switched off, Errors, else Options), modules match their short
/// label or their path. If some path or label contains the query, only those
/// are listed. Each group sorted by score, cut to its limit; an empty query
/// lists the start of each group.
pub fn palette_search(model: &Model, raw_query: &str) -> Vec<GroupResult> {
    let q: String = raw_query
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let a = &model.analysis;
    let overrides: HashSet<usize> = a.overrides.iter().copied().collect();
    let off: HashSet<usize> = a.switched_off.iter().map(|r| r.option).collect();
    let errors: HashSet<usize> = a.errors.iter().copied().collect();
    let group_of = |oi: usize| {
        if overrides.contains(&oi) {
            Group::Overrides
        } else if off.contains(&oi) {
            Group::Off
        } else if errors.contains(&oi) {
            Group::Errors
        } else {
            Group::Options
        }
    };
    let mut buckets: Vec<(Group, Vec<Item>)> = Group::ALL.iter().map(|&g| (g, Vec::new())).collect();
    let bucket = |b: &mut Vec<(Group, Vec<Item>)>, g: Group| -> usize { b.iter().position(|(x, _)| *x == g).unwrap() };
    let opt = |index, hits, score| Item {
        kind: Kind::Option,
        index,
        hits,
        place: Where::Label,
        score,
    };
    if q.is_empty() {
        let mut seen = HashSet::new();
        let mut add = |b: &mut Vec<(Group, Vec<Item>)>, g: Group, oi: usize| {
            if seen.insert(oi) {
                let k = bucket(b, g);
                b[k].1.push(opt(oi, vec![], 0.));
            }
        };
        for &oi in &a.overrides {
            add(&mut buckets, Group::Overrides, oi);
        }
        for r in &a.switched_off {
            add(&mut buckets, Group::Off, r.option);
        }
        for &oi in &a.errors {
            add(&mut buckets, Group::Errors, oi);
        }
        for &oi in &a.with_user {
            add(&mut buckets, Group::Options, oi);
        }
        let k = bucket(&mut buckets, Group::Modules);
        for (i, m) in model.modules().iter().enumerate() {
            if m.origin.is_userish() {
                buckets[k].1.push(Item {
                    kind: Kind::Module,
                    index: i,
                    hits: vec![],
                    place: Where::Label,
                    score: 0.,
                });
            }
        }
    } else {
        for (i, lower) in model.lower_paths.iter().enumerate() {
            if let Some(m) = fuzzy_match(&q, lower, &model.options()[i].path) {
                let k = bucket(&mut buckets, group_of(i));
                buckets[k].1.push(opt(i, m.hits, m.score));
            }
        }
        let k = bucket(&mut buckets, Group::Modules);
        for (i, m) in model.modules().iter().enumerate() {
            let label = module_short_label(m);
            let path = module_label(m);
            let found = match fuzzy_match(&q, &label.to_lowercase(), &label) {
                Some(ml) => Some((ml, Where::Label, 0.)),
                None => fuzzy_match(&q, &path.to_lowercase(), &path).map(|mp| (mp, Where::Path, 20.)),
            };
            if let Some((mt, place, penalty)) = found {
                buckets[k].1.push(Item {
                    kind: Kind::Module,
                    index: i,
                    hits: mt.hits,
                    place,
                    score: mt.score - penalty,
                });
            }
        }
        // Substring hits (scores near 1000) hide scattered subsequence ones.
        if buckets.iter().any(|(_, l)| l.iter().any(|it| it.score >= 500.)) {
            for (_, l) in buckets.iter_mut() {
                l.retain(|it| it.score >= 500.);
            }
        }
        for (_, l) in buckets.iter_mut() {
            l.sort_by(|x, y| y.score.total_cmp(&x.score).then(x.index.cmp(&y.index)));
        }
    }
    buckets
        .into_iter()
        .filter(|(_, l)| !l.is_empty())
        .map(|(g, mut l)| {
            let total = l.len();
            l.truncate(g.limit());
            GroupResult {
                group: g,
                total,
                items: l,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_beats_subsequence() {
        let a = fuzzy_match("fire", "networking.firewall.enable", "networking.firewall.enable").unwrap();
        assert_eq!(a.hits, vec![11, 12, 13, 14]);
        assert!(a.score > 1000.);
        let b = fuzzy_match("nfe", "networking.firewall.enable", "networking.firewall.enable").unwrap();
        assert!(b.score < 500.);
        assert_eq!(b.hits[0], 0);
        assert!(fuzzy_match("zzz", "abc", "abc").is_none());
    }

    #[test]
    fn camel_case_starts_count() {
        let m = fuzzy_match("lp", "boot.kernelpackages", "boot.kernelPackages").unwrap();
        // "l" of kernel ... "p" of Packages, a word start.
        assert_eq!(m.hits, vec![10, 11]);
    }
}
