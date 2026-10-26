//! Against the demo configuration's graph (tests/data/demo.json, from
//! `nix run . -- ./demo#nixosConfigurations.demo`) and the web viewer's
//! results on it (tests/data/demo.analysis.json, app/scripts/expect-analysis.js).

use std::path::Path;

use optgraph_core::{Model, Status, ValueView, load, module_label, module_short_label, omitted_badge, value_view};
use serde_json::Value;

fn data(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn demo() -> Model {
    Model::new(load::from_path(Path::new(&data("demo.json"))).expect("demo.json loads"))
}

fn expected() -> Value {
    serde_json::from_str(&std::fs::read_to_string(data("demo.analysis.json")).unwrap()).unwrap()
}

fn status_js(s: Status) -> &'static str {
    match s {
        Status::Win => "win",
        Status::Lose => "lose",
        Status::Off => "off",
        Status::Unknown => "unknown",
    }
}

fn indices(v: &Value) -> Vec<usize> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect()
}

#[test]
fn loads_the_demo() {
    let m = demo();
    let meta = &m.graph.meta;
    assert_eq!(meta.host.as_deref(), Some("demo"));
    assert!(meta.complete);
    assert!(meta.warnings.is_empty());
    assert_eq!(m.modules().len(), 27);
    assert_eq!(m.options().len(), 94);
}

#[test]
fn stats_and_lists_match_the_viewer() {
    let m = demo();
    let e = expected();
    let a = &m.analysis;
    let s = &e["stats"];
    assert_eq!(a.stats.modules as u64, s["modules"].as_u64().unwrap());
    assert_eq!(a.stats.options as u64, s["options"].as_u64().unwrap());
    assert_eq!(a.stats.definitions as u64, s["definitions"].as_u64().unwrap());
    assert_eq!(a.stats.overrides as u64, s["overrides"].as_u64().unwrap());
    assert_eq!(a.stats.switched_off as u64, s["switchedOff"].as_u64().unwrap());
    assert_eq!(a.stats.errors as u64, s["errors"].as_u64().unwrap());
    assert_eq!(a.overrides, indices(&e["overrides"]));
    assert_eq!(a.with_user, indices(&e["withUser"]));
    assert_eq!(a.nixpkgs_only, indices(&e["nixpkgsOnly"]));
    assert_eq!(a.errors, indices(&e["errors"]));
    let off: Vec<(usize, usize)> = a.switched_off.iter().map(|r| (r.option, r.definition)).collect();
    let off_js: Vec<(usize, usize)> = e["switchedOff"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize))
        .collect();
    assert_eq!(off, off_js);
}

#[test]
fn ladders_and_reasons_match_the_viewer() {
    let m = demo();
    let e = expected();
    for (oi, eo) in e["options"].as_array().unwrap().iter().enumerate() {
        let o = &m.options()[oi];
        assert_eq!(o.path, eo["path"].as_str().unwrap());
        let info = &m.analysis.info[oi];
        assert_eq!(info.override_, eo["override"].as_bool().unwrap(), "{}", o.path);
        assert_eq!(info.user_vs_user, eo["userVsUser"].as_bool().unwrap(), "{}", o.path);
        assert_eq!(
            info.beats.as_ref().map(|b| b.by_origin()).as_deref(),
            eo["beats"].as_str(),
            "{}",
            o.path
        );
        assert_eq!(
            info.beats.as_ref().map(|b| b.by_module()).as_deref(),
            eo["beatsModules"].as_str(),
            "{}",
            o.path
        );
        assert_eq!(info.off, indices(&eo["off"]), "{}", o.path);
        let rows: Vec<(usize, &str, String)> = m
            .ladder(oi)
            .into_iter()
            .map(|r| (r.def, status_js(r.status), r.reason))
            .collect();
        let rows_js: Vec<(usize, &str, String)> = eo["ladder"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r[0].as_u64().unwrap() as usize,
                    r[1].as_str().unwrap(),
                    r[2].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(rows, rows_js, "{}", o.path);
    }
}

#[test]
fn module_labels_match_the_viewer() {
    let m = demo();
    let e = expected();
    for (mo, em) in m.modules().iter().zip(e["modules"].as_array().unwrap()) {
        assert_eq!(module_short_label(mo), em[0].as_str().unwrap());
        assert_eq!(module_label(mo), em[1].as_str().unwrap());
    }
}

#[test]
fn the_demo_story() {
    let m = demo();
    // mkForce in hardening.nix beats mkDefault in another module.
    let fw = m.option_index("networking.firewall.enable").unwrap();
    let rows = m.ladder(fw);
    assert_eq!(rows[0].status, Status::Win);
    assert_eq!(m.options()[fw].definitions[rows[0].def].priority, Some(50));
    assert_eq!(rows[1].status, Status::Lose);
    assert_eq!(rows[1].reason, "beaten by hardening.nix: 50 < 1000");
    assert_eq!(rows[2].reason, "beaten by hardening.nix: 50 < 1500");
    assert!(m.options()[fw].definitions[rows[2].def].is_default());

    // A merged list: several winners, nixpkgs ones counted as +N.
    let pk = m.option_index("environment.systemPackages").unwrap();
    let o = &m.options()[pk];
    assert!(o.winners.len() > 1);
    assert_eq!(m.merged_modules(pk), o.winners.len());
    assert_eq!(omitted_badge(o), Some(format!("+{} nixpkgs", o.omitted_total())));
    assert!(m.ladder(pk).iter().any(|r| r.status == Status::Off));

    // Switched off: the default applies.
    let pr = m.option_index("services.printing.enable").unwrap();
    let rows = m.ladder(pr);
    assert_eq!(rows[0].status, Status::Win);
    assert_eq!(rows[0].reason, "nothing stronger is set: the default applies");
    assert_eq!(rows[1].status, Status::Off);
    assert_eq!(
        value_view(&m.options()[pr].definitions[rows[1].def]),
        ValueView::Missing("not evaluated: switched off")
    );
    let defining = m.defining_modules(pr);
    assert_eq!(defining.len(), 1);
    assert_eq!(defining[0].1.status, Status::Off);
    assert_eq!(optgraph_core::ring_text(&defining[0].1), "mkIf false");

    // The rings of an override: winner first.
    let defining = m.defining_modules(fw);
    assert_eq!(defining.len(), 2);
    assert_eq!(optgraph_core::ring_text(&defining[0].1), "✓ 50 mkForce");
    assert_eq!(optgraph_core::ring_text(&defining[1].1), "1000 mkDefault");

    // An option without nixpkgs definitions has no badge.
    assert_eq!(omitted_badge(&m.options()[fw]), None);
}

#[test]
fn modules_index_options_and_importers() {
    let m = demo();
    let hardening = m
        .modules()
        .iter()
        .position(|x| x.file.ends_with("/demo/modules/core/hardening.nix"))
        .unwrap();
    let paths: Vec<&str> = m.mod_options[hardening]
        .iter()
        .map(|&oi| m.options()[oi].path.as_str())
        .collect();
    assert!(paths.contains(&"networking.firewall.enable"));
    let core = m
        .modules()
        .iter()
        .position(|x| x.file.ends_with("/demo/modules/core.nix"))
        .unwrap();
    assert_eq!(m.importers[hardening], vec![core]);
}

#[test]
fn palette_search_matches_the_viewer() {
    use optgraph_core::search::{Group, Kind, Where, palette_search};
    let m = demo();
    let e = expected();
    let key = |g: Group| match g {
        Group::Overrides => "overrides",
        Group::Off => "off",
        Group::Errors => "errors",
        Group::Options => "options",
        Group::Modules => "modules",
    };
    for case in e["palette"].as_array().unwrap() {
        let q = case[0].as_str().unwrap();
        let got = palette_search(&m, q);
        let want = case[1].as_array().unwrap();
        assert_eq!(got.len(), want.len(), "query {q:?}");
        for (g, w) in got.iter().zip(want) {
            assert_eq!(key(g.group), w[0].as_str().unwrap(), "query {q:?}");
            assert_eq!(g.total as u64, w[1].as_u64().unwrap(), "query {q:?}");
            let items = w[2].as_array().unwrap();
            assert_eq!(g.items.len(), items.len(), "query {q:?}");
            for (it, wi) in g.items.iter().zip(items) {
                let kind = if it.kind == Kind::Option { "option" } else { "module" };
                assert_eq!(kind, wi[0].as_str().unwrap(), "query {q:?}");
                assert_eq!(it.index as u64, wi[1].as_u64().unwrap(), "query {q:?}");
                assert_eq!(it.hits, indices(&wi[2]), "query {q:?}");
                if it.kind == Kind::Module {
                    let place = if it.place == Where::Label { "label" } else { "path" };
                    assert_eq!(place, wi[3].as_str().unwrap(), "query {q:?}");
                }
                assert!((it.score - wi[4].as_f64().unwrap()).abs() < 1e-9, "query {q:?}");
            }
        }
    }
}
