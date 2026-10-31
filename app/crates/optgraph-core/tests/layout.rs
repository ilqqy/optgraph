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
        .map(|s| Size {
            w: s["w"].as_f64().unwrap(),
            h: s["h"].as_f64().unwrap(),
        })
        .collect();
    let edges: Vec<(usize, usize)> = e["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_u64().unwrap() as usize, p[1].as_u64().unwrap() as usize))
        .collect();
    let got = layered_layout(&sizes, &edges);
    let out = &e["out"];
    let layer: Vec<usize> = out["layer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(got.layer, layer, "{name}: layers");
    assert_eq!(
        got.crossings as u64,
        out["crossings"].as_u64().unwrap(),
        "{name}: crossings"
    );
    for (i, (x, y)) in got.x.iter().zip(&got.y).enumerate() {
        let (ex, ey) = (out["x"][i].as_f64().unwrap(), out["y"][i].as_f64().unwrap());
        assert!(
            close(*x, ex) && close(*y, ey),
            "{name}: node {i} at ({x}, {y}), layout.js ({ex}, {ey})"
        );
    }
    for (k, p) in got.paths.iter().enumerate() {
        let ep = &out["paths"][k];
        match p {
            None => assert!(ep.is_null(), "{name}: edge {k}"),
            Some(pts) => {
                let ep = ep.as_array().unwrap();
                assert_eq!(pts.len(), ep.len(), "{name}: edge {k}");
                for (pt, q) in pts.iter().zip(ep) {
                    assert!(
                        close(pt.x, q["x"].as_f64().unwrap()) && close(pt.y, q["y"].as_f64().unwrap()),
                        "{name}: edge {k}"
                    );
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

mod clusters {
    use optgraph_core::layout::{
        CLUSTER_HEAD, CLUSTER_PAD, ClusterBox, ClusterSpec, Size, bundled_links, cluster_on_screen, clustered_layout,
        layered_layout,
    };

    const S: Size = Size { w: 100., h: 28. };

    #[test]
    fn without_clusters_it_is_the_layered_layout() {
        let sizes = [S; 4];
        let edges = [(0, 1), (1, 2), (0, 3)];
        let plain = layered_layout(&sizes, &edges);
        let c = clustered_layout(&sizes, &edges, &[]);
        assert_eq!((c.x, c.y, c.paths), (plain.x, plain.y, plain.paths));
        assert!(c.boxes.is_empty());
    }

    #[test]
    fn members_sit_inside_their_box_and_boxes_do_not_overlap() {
        // 0 owns cluster {1, 2} (1 imports 2); 3 owns cluster {4}; 2 owns {5, 6}.
        let sizes = [S; 7];
        let edges = [(0, 1), (0, 2), (1, 2), (3, 4), (2, 5), (2, 6)];
        let spec = |members: &[usize]| ClusterSpec {
            members: members.to_vec(),
            label_w: 60.,
        };
        let c = clustered_layout(&sizes, &edges, &[spec(&[1, 2]), spec(&[4]), spec(&[5, 6])]);
        assert_eq!(c.boxes.len(), 3);
        for (b, members) in c.boxes.iter().zip([vec![1, 2], vec![4], vec![5, 6]]) {
            for m in members {
                assert!(
                    c.x[m] - S.w / 2. >= b.x + CLUSTER_PAD - 1e-6,
                    "member {m} left of its box"
                );
                assert!(c.x[m] + S.w / 2. <= b.x + b.w - CLUSTER_PAD + 1e-6);
                assert!(
                    c.y[m] - S.h / 2. >= b.y + CLUSTER_HEAD + CLUSTER_PAD - 1e-6,
                    "member {m} in the label row"
                );
                assert!(c.y[m] + S.h / 2. <= b.y + b.h - CLUSTER_PAD + 1e-6);
            }
        }
        for i in 0..3 {
            for j in i + 1..3 {
                let (p, q) = (c.boxes[i], c.boxes[j]);
                let apart = p.x + p.w <= q.x || q.x + q.w <= p.x || p.y + p.h <= q.y || q.y + q.h <= p.y;
                assert!(apart, "boxes {i} and {j} overlap");
            }
        }
        // Free nodes stay outside every box.
        for v in [0, 3] {
            for b in &c.boxes {
                let inside = c.x[v] > b.x && c.x[v] < b.x + b.w && c.y[v] > b.y && c.y[v] < b.y + b.h;
                assert!(!inside, "node {v} is inside a box");
            }
        }
    }

    #[test]
    fn edges_attach_to_the_box_and_leave_from_the_member() {
        let sizes = [S; 5];
        let edges = [(0, 1), (0, 2), (1, 2), (2, 3), (2, 4)];
        let spec = |members: &[usize]| ClusterSpec {
            members: members.to_vec(),
            label_w: 0.,
        };
        let c = clustered_layout(&sizes, &edges, &[spec(&[1, 2]), spec(&[3, 4])]);
        let into = c.paths[0].as_ref().unwrap();
        let b0 = c.boxes[0];
        let end = into.last().unwrap();
        assert!((end.x - b0.x).abs() < 1e-6, "ends on the box's left side");
        assert!((end.y - (b0.y + b0.h / 2.)).abs() < 1e-6);
        // The internal edge runs between the members.
        let inner = c.paths[2].as_ref().unwrap();
        assert!((inner[0].x - (c.x[1] + S.w / 2.)).abs() < 1e-6);
        assert!((inner.last().unwrap().x - (c.x[2] - S.w / 2.)).abs() < 1e-6);
        // Out of a cluster: from the member's right side to the other box.
        let out = c.paths[3].as_ref().unwrap();
        assert!((out[0].x - (c.x[2] + S.w / 2.)).abs() < 1e-6);
        assert!((out[0].y - c.y[2]).abs() < 1e-6);
        assert!((out.last().unwrap().x - c.boxes[1].x).abs() < 1e-6);
    }

    #[test]
    fn a_pinned_cluster_grows_to_its_label() {
        let b = ClusterBox {
            x: 100.,
            y: 100.,
            w: 120.,
            h: 80.,
        };
        // At full zoom, or without pins, the box is the box.
        let plain = cluster_on_screen(b, (0., 0., 0.5), 200., &[], 1.);
        assert_eq!((plain.x, plain.y, plain.w, plain.h), (50., 50., 60., 40.));
        assert_eq!(plain.head, CLUSTER_HEAD * 0.5);
        let full = cluster_on_screen(b, (0., 0., 1.), 200., &[(160., 150., 100., 28.)], 1.);
        assert_eq!((full.w, full.h), (120., 80.));
        // Zoomed out with a pin: wide enough for the full-size label, and
        // around the pin, whose label row sits above it.
        let pin = (160., 150., 100., 28.);
        let s = cluster_on_screen(b, (0., 0., 0.5), 200., &[pin], 1.);
        assert_eq!(s.head, CLUSTER_HEAD);
        assert!(s.w >= 200., "the label overruns: {}", s.w);
        let (cx, cy) = (160. * 0.5, 150. * 0.5);
        assert!(s.x <= cx - 50. && s.x + s.w >= cx + 50.);
        assert!(s.y + s.head <= cy - 14., "the label row covers the pin");
        assert!(s.y + s.h >= cy + 14.);
        // A short label does not widen it beyond the pin.
        let short = cluster_on_screen(b, (0., 0., 0.5), 10., &[pin], 1.);
        assert!(short.w < s.w);
    }

    #[test]
    fn large_owner_fans_are_bundled() {
        // Owner 0 has four links (one kept), owner 1 has two; a link that is
        // no owner link is never bundled.
        let links = [
            Some((0, false)),
            Some((0, true)),
            Some((1, false)),
            Some((0, false)),
            None,
            Some((0, false)),
            Some((1, false)),
        ];
        assert_eq!(
            bundled_links(&links, 3),
            vec![true, false, false, true, false, true, false]
        );
        assert_eq!(bundled_links(&links, 4), vec![false; 7]);
    }

    #[test]
    fn the_box_is_as_wide_as_its_label() {
        let c = clustered_layout(
            &[S],
            &[],
            &[ClusterSpec {
                members: vec![0],
                label_w: 400.,
            }],
        );
        assert_eq!(c.boxes[0].w, 400. + 2. * CLUSTER_PAD);
    }
}
