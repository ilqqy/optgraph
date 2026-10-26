//! layout.rs against the web viewer's layout.js on the same input
//! (tests/data/demo.layout.json, app/scripts/expect-layout.js): the demo's
//! import graph, a small graph with a cycle, a long edge, a self loop and
//! isolated nodes, and a dense random graph with crossings.

use std::path::Path;

use optgraph_core::layout::{Size, layered_layout};
use serde_json::Value;

fn expected() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/demo.layout.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * (1. + a.abs().max(b.abs()))
}

fn check(name: &str) {
    let e = &expected()[name];
    let sizes: Vec<Size> = e["sizes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| Size { w: s["w"].as_f64().unwrap(), h: s["h"].as_f64().unwrap() })
        .collect();
    let edges: Vec<(usize, usize)> = e["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize))
        .collect();
    let got = layered_layout(&sizes, &edges);
    let out = &e["out"];
    let layer: Vec<usize> = out["layer"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
    assert_eq!(got.layer, layer, "{name}: layers");
    assert_eq!(got.crossings as u64, out["crossings"].as_u64().unwrap(), "{name}: crossings");
    for (i, (x, y)) in got.x.iter().zip(&got.y).enumerate() {
        let (ex, ey) = (out["x"][i].as_f64().unwrap(), out["y"][i].as_f64().unwrap());
        assert!(close(*x, ex) && close(*y, ey), "{name}: node {i} at ({x}, {y}), layout.js ({ex}, {ey})");
    }
    for (k, p) in got.paths.iter().enumerate() {
        let ep = &out["paths"][k];
        match p {
            None => assert!(ep.is_null(), "{name}: edge {k}"),
            Some(pts) => {
                let ep = ep.as_array().unwrap();
                assert_eq!(pts.len(), ep.len(), "{name}: edge {k}");
                for (pt, q) in pts.iter().zip(ep) {
                    assert!(close(pt.x, q["x"].as_f64().unwrap()) && close(pt.y, q["y"].as_f64().unwrap()), "{name}: edge {k}");
                }
            }
        }
    }
}

#[test]
fn demo_matches_layout_js() {
    check("demo");
}

#[test]
fn cycle_long_edge_and_isolated_match_layout_js() {
    check("synthetic");
}

#[test]
fn dense_graph_matches_layout_js() {
    check("dense");
}

#[test]
fn empty_and_single() {
    let l = layered_layout(&[], &[]);
    assert!(l.x.is_empty());
    let l = layered_layout(&[Size { w: 10., h: 28. }], &[(0, 0)]);
    assert_eq!(l.layer, vec![0]);
    assert_eq!(l.paths, vec![None]);
    assert_eq!((l.x[0], l.y[0]), (5., 14.));
}
