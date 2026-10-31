//! Formatting helpers shared with the web viewer (viewer/src/util.js): store
//! paths shortened, priority names, counts.

/// `/nix/store/<hash>-source/nixos/modules/x.nix` -> `…/nixos/modules/x.nix`;
/// a bare store path gives its name. Anything else is returned unchanged.
pub fn short_path(p: &str) -> String {
    match store_prefix_len(p) {
        None => p.to_string(),
        Some(n) => {
            let rest = &p[n..];
            if rest.is_empty() {
                p[44..].to_string()
            } else {
                format!("…{rest}")
            }
        }
    }
}

/// Length of `/nix/store/<32 chars [0-9a-z]>-<name>` at the start of `p`.
fn store_prefix_len(p: &str) -> Option<usize> {
    let rest = p.strip_prefix("/nix/store/")?;
    let b = rest.as_bytes();
    if b.len() < 34 || !b[..32].iter().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase()) || b[32] != b'-' {
        return None;
    }
    let name = &rest[33..];
    let name_len = name.find('/').unwrap_or(name.len());
    if name_len == 0 {
        return None;
    }
    Some(11 + 33 + name_len)
}

/// The last `n` segments: `…/networking/firewall.nix`.
pub fn tail_path(p: &str, n: usize) -> String {
    let s = short_path(p);
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() > n + 1 {
        format!("…/{}", parts[parts.len() - n..].join("/"))
    } else {
        s
    }
}

pub fn base_name(p: &str) -> String {
    let s = short_path(p);
    match s.rfind('/') {
        Some(i) => s[i + 1..].to_string(),
        None => s,
    }
}

/// An option path cut in the middle: `text` is at most `max` characters. The
/// first `head` characters of the path are kept, then `…`, then the path from
/// character `tail` on. A path that fits has `head == tail == its length`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elided {
    pub text: String,
    pub head: usize,
    pub tail: usize,
}

impl Elided {
    /// Where character `ci` of the path went in `text`; `None` if cut out.
    pub fn map_index(&self, ci: usize) -> Option<usize> {
        if ci < self.head {
            Some(ci)
        } else if ci >= self.tail {
            Some(ci - self.tail + self.head + usize::from(self.tail > self.head))
        } else {
            None
        }
    }
}

/// Cuts a long option path in the middle, keeping its end: the last segments
/// (the option's name and what holds it) stay readable. Both cuts prefer a
/// segment boundary (a dot outside quotes); a long quoted name keeps its end.
pub fn elide_middle(path: &str, max: usize) -> Elided {
    let chars: Vec<char> = path.chars().collect();
    let n = chars.len();
    if n <= max {
        return Elided {
            text: path.to_string(),
            head: n,
            tail: n,
        };
    }
    // Character indices just after each dot outside quotes.
    let mut starts = Vec::new();
    let mut quoted = false;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '"' => quoted = !quoted,
            '.' if !quoted => starts.push(i + 1),
            _ => {}
        }
    }
    let room = max.saturating_sub(1).max(1);
    let budget = (room * 2 / 3).max(1);
    let raw_tail = n - budget.min(n);
    let tail = starts
        .iter()
        .copied()
        .find(|&s| s >= raw_tail && n - s >= budget / 2)
        .unwrap_or(raw_tail);
    let tail_len = n - tail;
    let raw_head = room.saturating_sub(tail_len);
    // End the head after a dot, unless that loses more than a third of it.
    let head = starts
        .iter()
        .rev()
        .copied()
        .find(|&s| s <= raw_head && s * 3 >= raw_head * 2)
        .unwrap_or(raw_head);
    let mut text: String = chars[..head].iter().collect();
    text.push('…');
    text.extend(&chars[tail..]);
    Elided { text, head, tail }
}

/// A path broken into two lines of at most `per_line` characters, after a dot
/// or a slash, when such a break exists; otherwise unchanged (the renderer then
/// wraps it wherever it must).
pub fn break_lines(text: &str, per_line: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n <= per_line {
        return text.to_string();
    }
    let at = (n.saturating_sub(per_line).max(1)..=per_line.min(n - 1))
        .rev()
        .find(|&i| matches!(chars[i - 1], '.' | '/') || chars[i] == '/' || chars[i] == '…');
    match at {
        Some(i) => format!(
            "{}\n{}",
            chars[..i].iter().collect::<String>(),
            chars[i..].iter().collect::<String>()
        ),
        None => text.to_string(),
    }
}

/// The name of a well-known priority.
pub fn priority_name(p: i64) -> Option<&'static str> {
    Some(match p {
        10 => "mkVMOverride",
        50 => "mkForce",
        60 => "mkImageMediaOverride",
        100 => "normal",
        1000 => "mkDefault",
        1500 => "default",
        _ => return None,
    })
}

/// "50 mkForce", "75 mkOverride", "? unknown".
pub fn priority_label(p: Option<i64>) -> String {
    match p {
        None => "? unknown".into(),
        Some(p) => match priority_name(p) {
            Some(n) => format!("{p} {n}"),
            None => format!("{p} mkOverride"),
        },
    }
}

/// Name only: "mkForce", "normal", "mkOverride 75", "unknown".
pub fn prio_name(p: Option<i64>) -> String {
    match p {
        None => "unknown".into(),
        Some(p) => priority_name(p)
            .map(str::to_string)
            .unwrap_or_else(|| format!("mkOverride {p}")),
    }
}

/// 12345 -> "12,345".
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// plural(2, "module", None) -> "2 modules"; plural(2, "is", Some("are")) -> "2 are".
pub fn plural(n: u64, word: &str, many: Option<&str>) -> String {
    let w = if n == 1 {
        word.to_string()
    } else {
        many.map(str::to_string).unwrap_or_else(|| format!("{word}s"))
    };
    format!("{} {w}", thousands(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORE: &str = "/nix/store/0123456789abcdefghijklmnopqrstuv-source";

    #[test]
    fn short_paths() {
        assert_eq!(
            short_path(&format!("{STORE}/demo/modules/core.nix")),
            "…/demo/modules/core.nix"
        );
        assert_eq!(short_path(STORE), "source");
        assert_eq!(
            short_path("/etc/nixos/configuration.nix"),
            "/etc/nixos/configuration.nix"
        );
        assert_eq!(
            short_path("/nix/store/short-source/x.nix"),
            "/nix/store/short-source/x.nix"
        );
        assert_eq!(base_name(&format!("{STORE}/demo/modules/core.nix")), "core.nix");
        assert_eq!(base_name(&format!("{STORE}/demo/flake.nix:23")), "flake.nix:23");
        assert_eq!(tail_path(&format!("{STORE}/nixos/modules/x/y.nix"), 2), "…/x/y.nix");
        assert_eq!(tail_path("a/b.nix", 2), "a/b.nix");
    }

    #[test]
    fn priorities() {
        assert_eq!(priority_label(Some(50)), "50 mkForce");
        assert_eq!(priority_label(Some(75)), "75 mkOverride");
        assert_eq!(priority_label(None), "? unknown");
        assert_eq!(prio_name(Some(1000)), "mkDefault");
        assert_eq!(prio_name(Some(1500)), "default");
        assert_eq!(prio_name(Some(100)), "normal");
        assert_eq!(prio_name(Some(75)), "mkOverride 75");
        assert_eq!(prio_name(None), "unknown");
    }

    #[test]
    fn eliding_paths() {
        let p = "home-manager.users.me.programs.zen-browser.profiles.default.settings";
        // Fits: unchanged.
        let e = elide_middle(p, 200);
        assert_eq!((e.text.as_str(), e.head, e.tail), (p, p.len(), p.len()));
        assert_eq!(e.map_index(3), Some(3));
        // Both cuts at segment boundaries, the end kept.
        let e = elide_middle(p, 50);
        assert_eq!(e.text, "home-manager.users.me.…profiles.default.settings");
        assert!(e.text.chars().count() <= 50);
        assert!(e.text.ends_with(".default.settings"));
        // A long quoted name keeps its end; dots inside quotes are not boundaries.
        let q = "fixture.home.zoe.files.\"config/some-application/with/a/rather/long/path/settings.conf\".text";
        let e = elide_middle(q, 54);
        assert!(e.text.chars().count() <= 54, "{}", e.text);
        assert!(e.text.ends_with("settings.conf\".text"), "{}", e.text);
        assert!(e.text.starts_with("fixture.home.zoe."), "{}", e.text);
        // Indices of the path map into the text, or are cut out.
        let chars: Vec<char> = e.text.chars().collect();
        let orig: Vec<char> = q.chars().collect();
        for (ci, c) in orig.iter().enumerate() {
            if let Some(t) = e.map_index(ci) {
                assert_eq!(chars[t], *c, "index {ci}");
            }
        }
        assert_eq!(e.map_index(e.head), None);
        // No dots at all, and tiny limits.
        let e = elide_middle("abcdefghijklmnopqrstuvwxyz", 11);
        assert_eq!(e.text.chars().count(), 11);
        assert!(e.text.contains('…'));
        assert!(elide_middle("abcdefgh", 2).text.chars().count() <= 2);
    }

    #[test]
    fn breaking_lines_at_boundaries() {
        assert_eq!(break_lines("users.users.alice", 30), "users.users.alice");
        assert_eq!(
            break_lines("home-manager.users.ilyanix", 23),
            "home-manager.users.\nilyanix"
        );
        let b = break_lines("home-manager.users.…mozilla.firefoxNativeMessagingHosts", 29);
        assert_eq!(b, "home-manager.users.…mozilla.\nfirefoxNativeMessagingHosts");
        // The second line must fit too; no boundary at all stays unbroken.
        assert_eq!(
            break_lines("abcdefghijklmnopqrstuvwxyz", 13),
            "abcdefghijklmnopqrstuvwxyz"
        );
        let q = "fixture.home.zoe.…/rather/long/path/settings.conf\".text";
        for line in break_lines(q, 29).lines() {
            assert!(line.chars().count() <= 29, "{line}");
        }
    }

    #[test]
    fn counts() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(plural(1, "module", None), "1 module");
        assert_eq!(plural(2, "module", None), "2 modules");
        assert_eq!(plural(2, "definition is", Some("definitions are")), "2 definitions are");
    }
}
