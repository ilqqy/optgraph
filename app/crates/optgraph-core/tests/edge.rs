//! Edge cases the demo doesn't have: errors, unknown priorities, mkIf
//! errors, redacted and missing previews, a definition with its own file,
//! a default without a declaration, inputs, bad documents.

use optgraph_core::{Model, Origin, Status, ValueView, load, module_label, module_short_label, value_view};

const S: &str = "/nix/store/0123456789abcdefghijklmnopqrstuv-source";

fn graph() -> String {
    format!(
        r#"{{
  "meta": {{ "host": null, "nixpkgsRev": null, "nixpkgsVersion": "26.11.1", "generatedAt": null,
            "toolVersion": "0.1.0+test", "scope": "all", "attribution": "file-based", "complete": false,
            "warnings": [{{ "code": "eval-crash", "message": "boom", "subject": "x.aborting" }},
                         {{ "code": "some-future-code", "message": "new", "subject": null }}] }},
  "modules": [
    {{ "id": "{S}/a.nix", "file": "{S}/a.nix", "imports": ["{S}/b.nix", ":anon-9:anon-1"], "origin": "user",
       "disabled": false, "modulesIndex": null, "position": null }},
    {{ "id": "{S}/b.nix", "file": "{S}/b.nix", "imports": [], "origin": "input:extra/nested",
       "disabled": true, "modulesIndex": null, "position": null }},
    {{ "id": ":anon-9:anon-1", "file": "{S}/a.nix", "imports": [], "origin": "weird",
       "disabled": false, "modulesIndex": null, "position": "{S}/a.nix:7" }},
    {{ "id": ":anon-12", "file": "{S}/flake.nix", "imports": [], "origin": "user-inline",
       "disabled": false, "modulesIndex": 3, "position": "{S}/mine/flake.nix:20" }},
    {{ "id": "{S}/nixos/x.nix", "file": "{S}/nixos/x.nix", "imports": [], "origin": "nixpkgs",
       "disabled": false, "modulesIndex": null, "position": null }}
  ],
  "options": [
    {{ "path": "x.aborting", "loc": ["x", "aborting"], "declaredIn": [], "type": null, "definitions": [],
       "winners": [], "highestPrio": null, "omitted": {{ "nixpkgsActive": 0, "nixpkgsInactive": 0 }},
       "error": "excluded after an uncatchable crash during reconstruct (see meta.warnings)" }},
    {{ "path": "x.password", "loc": ["x", "password"], "declaredIn": ["{S}/a.nix"], "type": "string",
       "definitions": [
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": 100, "active": true, "condition": null, "valuePreview": "<redacted>" }},
         {{ "kind": "definition", "file": "{S}/own.nix", "module": ":anon-12", "priority": 1000, "active": true, "condition": null, "valuePreview": "\"x\"" }},
         {{ "kind": "default", "file": null, "module": null, "priority": 1500, "active": true, "condition": null, "valuePreview": null }}
       ],
       "winners": [0], "highestPrio": 100, "omitted": {{ "nixpkgsActive": 2, "nixpkgsInactive": 1234 }}, "error": null }},
    {{ "path": "x.throws", "loc": ["x", "throws"], "declaredIn": ["{S}/a.nix"], "type": "string",
       "definitions": [
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": null, "active": true, "condition": null, "valuePreview": null }},
         {{ "kind": "definition", "module": ":anon-9:anon-1", "priority": 100, "active": true, "condition": null, "valuePreview": "1" }},
         {{ "kind": "definition", "module": ":anon-12", "priority": null, "active": false, "condition": "mkIf-error", "valuePreview": null }}
       ],
       "winners": [], "highestPrio": null, "omitted": {{ "nixpkgsActive": 0, "nixpkgsInactive": 0 }},
       "error": "definition 0: throw/assert while reading its value; winner unknown: a definition could not be classified" }},
    {{ "path": "x.vs", "loc": ["x", "vs"], "declaredIn": [], "type": "string",
       "definitions": [
         {{ "kind": "definition", "module": "{S}/nixos/x.nix", "priority": 50, "active": true, "condition": null, "valuePreview": "1" }},
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": 75, "active": true, "condition": null, "valuePreview": "2" }},
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": null, "active": false, "condition": "mkIf-false", "valuePreview": null }}
       ],
       "winners": [0], "highestPrio": 50, "omitted": {{ "nixpkgsActive": 0, "nixpkgsInactive": 0 }}, "error": null }},
    {{ "path": "x.zlist", "loc": ["x", "zlist"], "declaredIn": [], "type": "list of string",
       "definitions": [
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": 1000, "active": true, "condition": null, "valuePreview": "[ \"a\" ]" }},
         {{ "kind": "default", "file": null, "module": null, "priority": 1500, "active": true, "condition": null, "valuePreview": "[ ]" }}
       ],
       "winners": [], "highestPrio": 100,
       "omitted": {{ "nixpkgsActive": 4, "nixpkgsInactive": 7, "nixpkgsWinners": 3 }}, "error": null }},
    {{ "path": "x.zmerged", "loc": ["x", "zmerged"], "declaredIn": [], "type": "attribute set",
       "definitions": [
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": null, "active": false, "condition": "mkIf-false", "valuePreview": null }},
         {{ "kind": "default", "file": null, "module": null, "priority": 1500, "active": true, "condition": null, "valuePreview": "{{ }}" }}
       ],
       "winners": [], "highestPrio": 100, "omitted": {{ "nixpkgsActive": 11, "nixpkgsInactive": 95 }}, "error": null }},
    {{ "path": "x.zmix", "loc": ["x", "zmix"], "declaredIn": [], "type": "list of string",
       "definitions": [
         {{ "kind": "definition", "module": "{S}/a.nix", "priority": 100, "active": true, "condition": null, "valuePreview": "[ \"a\" ]" }},
         {{ "kind": "definition", "module": ":anon-12", "priority": 1000, "active": true, "condition": null, "valuePreview": "[ \"b\" ]" }}
       ],
       "winners": [0], "highestPrio": 100,
       "omitted": {{ "nixpkgsActive": 2, "nixpkgsInactive": 0, "nixpkgsWinners": 2 }}, "error": null }}
  ],
  "futureField": true
}}"#
    )
}

fn model() -> Model {
    Model::new(load::from_str(&graph()).expect("loads"))
}

#[test]
fn origins_and_labels() {
    let m = model();
    let ms = m.modules();
    assert_eq!(ms[1].origin, Origin::Input("extra/nested".into()));
    assert_eq!(ms[1].origin.label(), "input:extra/nested");
    assert_eq!(ms[2].origin, Origin::Unknown);
    assert_eq!(module_short_label(&ms[2]), "a.nix:7 (anon)");
    assert_eq!(module_label(&ms[2]), "…/a.nix:7 (anonymous)");
    assert_eq!(module_short_label(&ms[3]), "inline #3");
    assert_eq!(module_label(&ms[3]), "inline module #3 (flake.nix:20)");
    assert!(!ms[4].origin.is_userish());
    assert_eq!(m.importers[1], vec![0]);
}

#[test]
fn redacted_own_file_and_default_without_declaration() {
    let m = model();
    let oi = m.option_index("x.password").unwrap();
    let o = &m.options()[oi];
    assert_eq!(value_view(&o.definitions[0]), ValueView::Redacted);
    assert_eq!(value_view(&o.definitions[1]), ValueView::Preview("\"x\"".into()));
    assert_eq!(value_view(&o.definitions[2]), ValueView::Missing("no preview"));
    // Absent file: the module's; own file: its own; default with null: none.
    assert_eq!(
        m.def_file(&o.definitions[0]).as_deref(),
        Some(format!("{S}/a.nix").as_str())
    );
    assert_eq!(
        m.def_file(&o.definitions[1]).as_deref(),
        Some(format!("{S}/own.nix").as_str())
    );
    assert_eq!(m.def_file(&o.definitions[2]), None);
    assert_eq!(m.def_who(&o.definitions[2]), "the default");
    assert_eq!(optgraph_core::omitted_badge(o).as_deref(), Some("+1,236 nixpkgs"));
    let rows = m.ladder(oi);
    assert_eq!(rows[0].reason, "beats inline #3: 100 < 1000");
    assert_eq!(rows[1].reason, "beaten by a.nix: 100 < 1000");
    assert_eq!(rows[2].reason, "beaten by a.nix: 100 < 1500");
}

#[test]
fn errors_and_unknown_winners() {
    let m = model();
    assert_eq!(m.analysis.errors, vec![0, 2]);
    assert_eq!(m.analysis.stats.errors, 2);
    let oi = m.option_index("x.throws").unwrap();
    let rows = m.ladder(oi);
    let st: Vec<(usize, Status, &str)> = rows.iter().map(|r| (r.def, r.status, r.reason.as_str())).collect();
    assert_eq!(
        st,
        vec![
            (1, Status::Unknown, "the winner is unknown (see the error)"),
            (0, Status::Unknown, "reading its value threw: priority unknown"),
            (2, Status::Off, "its mkIf condition threw or is not a bool"),
        ]
    );
    let o = &m.options()[oi];
    assert_eq!(
        value_view(&o.definitions[0]),
        ValueView::Missing("value threw (see the error)")
    );
    // No known winner: not an override, and no crash on the missing winner.
    assert!(!m.analysis.info[oi].override_);
    // mkIf-error is not "switched off" in the lists (only mkIf-false is).
    assert!(m.analysis.info[oi].off.is_empty());
    let ring = m.defining_modules(oi);
    assert!(ring.iter().any(|(_, s)| optgraph_core::ring_text(s) == "mkIf error"));
    assert!(
        ring.iter()
            .any(|(_, s)| optgraph_core::ring_text(s) == "? priority unknown")
    );
    assert!(ring.iter().any(|(_, s)| optgraph_core::ring_text(s) == "? 100 normal"));
}

#[test]
fn nixpkgs_beats_user_with_a_custom_priority() {
    let m = model();
    let oi = m.option_index("x.vs").unwrap();
    let info = &m.analysis.info[oi];
    assert!(info.override_);
    assert!(!info.user_vs_user);
    assert_eq!(
        info.beats.as_ref().unwrap().by_origin(),
        "mkForce (nixpkgs) beats mkOverride 75 (user)"
    );
    assert_eq!(info.off, vec![2]);
    assert_eq!(m.analysis.switched_off.len(), 2); // and x.zmerged's
}

#[test]
fn meta_and_bad_documents() {
    let m = model();
    let meta = &m.graph.meta;
    assert_eq!(meta.scope.as_str(), "all");
    assert_eq!(meta.attribution.as_str(), "file-based");
    assert_eq!(meta.warnings[1].code, "some-future-code");
    assert!(
        load::from_str("{")
            .unwrap_err()
            .to_string()
            .starts_with("not valid JSON")
    );
    let e = load::from_str(r#"{"meta": {}, "modules": []}"#)
        .unwrap_err()
        .to_string();
    assert!(e.starts_with("not an optgraph graph.json"), "{e}");
    assert!(load::from_path(std::path::Path::new("/nonexistent/graph.json")).is_err());
}

fn ladder_of(m: &Model, path: &str) -> Vec<(usize, Status, String)> {
    let oi = m.option_index(path).unwrap();
    m.ladder(oi).into_iter().map(|r| (r.def, r.status, r.reason)).collect()
}

#[test]
fn winners_all_omitted() {
    // Every winner is a nixpkgs definition counted in `omitted`: `winners`
    // is empty, `nixpkgsWinners` says how many win at highestPrio.
    let m = model();
    let oi = m.option_index("x.zlist").unwrap();
    let o = &m.options()[oi];
    assert_eq!((o.hidden_winners(), o.winner_count()), (3, 3));
    assert_eq!(
        ladder_of(&m, "x.zlist"),
        vec![
            (
                0,
                Status::Lose,
                "beaten by 3 nixpkgs definitions at 100 (not listed): 100 < 1000".into()
            ),
            (
                1,
                Status::Lose,
                "beaten by 3 nixpkgs definitions at 100 (not listed): 100 < 1500".into()
            ),
        ]
    );
    // The user's mkDefault lost to nixpkgs: an override.
    let info = &m.analysis.info[oi];
    assert!(info.override_);
    assert!(!info.user_vs_user);
    assert_eq!(
        info.beats.as_ref().unwrap().by_module(),
        "normal in nixpkgs (3 not listed) beats mkDefault in a.nix"
    );
}

#[test]
fn listed_and_omitted_winners() {
    // One listed winner merged with omitted nixpkgs ones.
    let m = model();
    assert_eq!(
        ladder_of(&m, "x.zmix"),
        vec![
            (
                0,
                Status::Win,
                "merged with 2 others at priority 100 (2 nixpkgs, not listed)".into()
            ),
            (
                1,
                Status::Lose,
                "beaten by 3 definitions at 100 (2 nixpkgs, not listed): 100 < 1000".into()
            ),
        ]
    );
    let oi = m.option_index("x.zmix").unwrap();
    assert_eq!(
        m.analysis.info[oi].beats.as_ref().unwrap().by_module(),
        "normal in a.nix beats mkDefault in inline #3"
    );
}

#[test]
fn winners_not_listed_in_an_older_graph() {
    // Before omitted.nixpkgsWinners: `winners` empty while highestPrio is
    // set, and nothing says who wins.
    let m = model();
    let oi = m.option_index("x.zmerged").unwrap();
    assert_eq!(m.options()[oi].omitted.nixpkgs_winners, 0);
    assert_eq!(
        ladder_of(&m, "x.zmerged"),
        vec![
            (1, Status::Unknown, "the winners are not listed".into()),
            (0, Status::Off, "switched off: its mkIf condition is false".into()),
        ]
    );
    assert!(!m.analysis.info[oi].override_);
}
