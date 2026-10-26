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
