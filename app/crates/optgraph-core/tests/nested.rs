//! A graph with nested options and modules (submodule instances): the
//! fixture's graph.json (tests/fixture, `--submodule-depth 3`). The new fields
//! are optional, so older graphs (tests/data/demo.json) still load.

use optgraph_core::{Model, Status, instance_name, load, module_label, module_short_label};

fn model() -> Model {
    Model::new(load::from_str(include_str!("data/nested.json")).expect("the nested graph loads"))
}

fn option(m: &Model, path: &str) -> usize {
    m.graph
        .options
        .iter()
        .position(|o| o.path == path)
        .unwrap_or_else(|| panic!("no option {path}"))
}

#[test]
fn nested_fields_load() {
    let m = model();
    assert_eq!(m.graph.meta.submodule_depth, Some(3));
    let o = &m.graph.options[option(&m, "fixture.accounts.dave.shell")];
    assert_eq!(o.instance.as_deref(), Some("fixture.accounts.dave"));
    assert_eq!(o.parent.as_deref(), Some("fixture.accounts"));
    let top = &m.graph.options[option(&m, "fixture.accounts")];
    assert!(top.instance.is_none() && top.parent.is_none());
    let nested: Vec<_> = m.modules().iter().filter(|x| x.scope.is_some()).collect();
    assert!(!nested.is_empty());
    assert!(nested.iter().any(|x| x.owner.is_some()));
    assert!(
        m.modules()
            .iter()
            .filter(|x| x.scope.is_none())
            .all(|x| x.owner.is_none())
    );
}

#[test]
fn nested_labels_are_readable() {
    let m = model();
    for x in m.modules() {
        for label in [module_label(x), module_short_label(x)] {
            assert!(!label.contains('«'), "raw nested id in a label: {label}");
        }
    }
    let dave = m
        .modules()
        .iter()
        .find(|x| x.id == "«fixture.accounts.dave»:anon-4")
        .expect("dave's module");
    assert_eq!(module_short_label(dave), "dave › nested-force.nix");
    assert!(module_label(dave).starts_with("fixture.accounts.dave › …/"));
    let extra = m
        .modules()
        .iter()
        .find(|x| x.scope.as_deref() == Some("fixture.home.alice") && x.id.ends_with("/alice-extra.nix"))
        .expect("the file imported inside alice");
    assert_eq!(module_short_label(extra), "alice › alice-extra.nix");
}

#[test]
fn instance_names() {
    assert_eq!(instance_name("users.users.alice"), "alice");
    assert_eq!(instance_name("fileSystems.\"/boot\""), "/boot");
    assert_eq!(
        instance_name("services.nginx.virtualHosts.\"a.b\".locations.\"/\""),
        "/"
    );
    assert_eq!(instance_name("fixture.single"), "single");
}

#[test]
fn shadowed_definitions_never_reached_the_instance() {
    let m = model();
    let oi = option(&m, "fixture.home.carol.theme");
    let o = &m.graph.options[oi];
    let shadowed = o
        .definitions
        .iter()
        .position(|d| d.shadowed_at.is_some())
        .expect("a shadowed definition");
    let row = m
        .ladder(oi)
        .into_iter()
        .find(|r| r.def == shadowed)
        .expect("its ladder row");
    assert_eq!(row.status, Status::Lose);
    assert_eq!(
        row.reason,
        "never reached the instance: the definition it is part of lost at fixture.home.carol"
    );
    // The winner's reason is about a real loser, not the shadowed one.
    let win = m
        .ladder(oi)
        .into_iter()
        .find(|r| r.status == Status::Win)
        .expect("a winner");
    assert!(!win.reason.contains("nested-plain"), "{}", win.reason);
}

#[test]
fn declaration_counters() {
    let m = model();
    let o = &m.graph.options[option(&m, "fixture.home.alice.theme")];
    assert_eq!(o.omitted.declaration_active, 1);
    assert_eq!(o.omitted_declaration_total(), 1);
    assert_eq!(o.hidden_winners(), 0);
}

#[test]
fn nested_modules_hang_off_their_owner() {
    let m = model();
    for x in m.modules().iter().filter(|x| x.owner.is_some()) {
        let owner = x.owner.as_deref().unwrap();
        assert!(
            m.modules().iter().any(|y| y.id == owner),
            "owner {owner} of {} is listed",
            x.id
        );
    }
}

#[test]
fn older_graphs_still_load() {
    let g = load::from_str(include_str!("data/demo.json")).expect("demo loads");
    assert!(g.meta.submodule_depth.is_none());
    assert!(
        g.options
            .iter()
            .all(|o| o.instance.is_none() && o.omitted.declaration_active == 0)
    );
}

// Parity with the web viewer on the same graph (tests/data/nested.analysis.json,
// app/scripts/expect-analysis.js): verdicts, ladders and module labels.
#[test]
fn nested_graph_matches_the_viewer() {
    let m = model();
    let e: serde_json::Value = serde_json::from_str(include_str!("data/nested.analysis.json")).unwrap();
    let status_js = |s: Status| match s {
        Status::Win => "win",
        Status::Lose => "lose",
        Status::Off => "off",
        Status::Unknown => "unknown",
    };
    for (oi, eo) in e["options"].as_array().unwrap().iter().enumerate() {
        let o = &m.graph.options[oi];
        assert_eq!(o.path, eo["path"].as_str().unwrap());
        let info = &m.analysis.info[oi];
        assert_eq!(info.override_, eo["override"].as_bool().unwrap(), "{}", o.path);
        assert_eq!(
            info.beats.as_ref().map(|b| b.by_module()).as_deref(),
            eo["beatsModules"].as_str(),
            "{}",
            o.path
        );
        let rows: Vec<(u64, String, String)> = m
            .ladder(oi)
            .into_iter()
            .map(|r| (r.def as u64, status_js(r.status).to_string(), r.reason))
            .collect();
        let rows_js: Vec<(u64, String, String)> = eo["ladder"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r[0].as_u64().unwrap(),
                    r[1].as_str().unwrap().into(),
                    r[2].as_str().unwrap().into(),
                )
            })
            .collect();
        assert_eq!(rows, rows_js, "{}", o.path);
    }
    for (mo, em) in m.modules().iter().zip(e["modules"].as_array().unwrap()) {
        assert_eq!(module_short_label(mo), em[0].as_str().unwrap());
        assert_eq!(module_label(mo), em[1].as_str().unwrap());
    }
}
