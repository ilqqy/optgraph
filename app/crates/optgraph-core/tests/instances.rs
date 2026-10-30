//! Instances, breadcrumbs and the palette's Instances group, against the
//! fixture's and the demo's graphs (tests/data/nested.json, demo-nested.json:
//! `nix run . -- ./tests/fixture#nixosConfigurations.test`, `./demo#...demo`).

use optgraph_core::instances::{Crumb, CrumbTarget, relative_to};
use optgraph_core::search::{Group, Kind, palette_search};
use optgraph_core::{Model, Status, load, parent_path};

fn fixture() -> Model {
    Model::new(load::from_str(include_str!("data/nested.json")).expect("loads"))
}

fn demo() -> Model {
    Model::new(load::from_str(include_str!("data/demo-nested.json")).expect("loads"))
}

fn oi(m: &Model, path: &str) -> usize {
    m.option_index(path).unwrap_or_else(|| panic!("no option {path}"))
}

fn inst(m: &Model, path: &str) -> usize {
    m.instances.index(path).unwrap_or_else(|| panic!("no instance {path}"))
}

fn crumbs(m: &Model, path: &str) -> Vec<String> {
    m.instances
        .breadcrumb(oi(m, path), path)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

#[test]
fn paths_split() {
    assert_eq!(parent_path("users.users.alice"), "users.users");
    assert_eq!(parent_path("fileSystems.\"/boot\""), "fileSystems");
    assert_eq!(parent_path("a.\"b.c\".d"), "a.\"b.c\"");
    assert_eq!(parent_path("a"), "");
    assert_eq!(relative_to("a.b.c", "a"), Some("b.c"));
    assert_eq!(relative_to("ab.c", "a"), None);
}

#[test]
fn the_demo_instances() {
    let m = demo();
    let is = &m.instances;
    assert!(!is.is_empty());
    let i = inst(&m, "users.users.demo");
    let d = &is.list[i];
    assert_eq!(d.name, "demo");
    assert_eq!(d.parent, "users.users");
    assert_eq!(d.parent_option, Some(oi(&m, "users.users")));
    assert_eq!(d.enclosing, None);
    assert_eq!(d.depth, 0);
    assert_eq!(d.options.len(), 4);
    assert_eq!(d.total_options, 4);
    assert_eq!(d.modules.len(), d.total_modules);
    assert!(
        d.modules
            .iter()
            .all(|&mi| m.modules()[mi].scope.as_deref() == Some("users.users.demo"))
    );
    // The shell's two definitions: an override; the podman group is switched off.
    assert_eq!(d.overrides, 1);
    assert_eq!(d.switched_off, 1);
    assert_eq!(d.errors, 0);
    assert!(!d.owners.is_empty());
    assert_eq!(is.nested_options(), 13);
    assert_eq!(is.top_level.len() + is.nested_options(), m.options().len());
    assert_eq!(
        crumbs(&m, "users.users.demo.shell"),
        vec!["users.users", "demo", "shell"]
    );
    assert!(crumbs(&m, "networking.firewall.enable").is_empty());
}

#[test]
fn every_nested_option_and_module_has_its_instance() {
    for m in [fixture(), demo()] {
        let is = &m.instances;
        for (i, o) in m.options().iter().enumerate() {
            assert_eq!(is.of_option[i].map(|k| is.list[k].path.as_str()), o.instance.as_deref());
        }
        for (i, x) in m.modules().iter().enumerate() {
            assert_eq!(is.of_module[i].map(|k| is.list[k].path.as_str()), x.scope.as_deref());
        }
        // Every option is either top-level or in exactly one instance.
        let listed: usize = is.list.iter().map(|x| x.options.len()).sum();
        assert_eq!(listed, is.nested_options());
        // The roots' totals add up to everything nested.
        let total: usize = is.roots.iter().map(|&r| is.list[r].total_options).sum();
        assert_eq!(total, is.nested_options());
        let mods: usize = is.roots.iter().map(|&r| is.list[r].total_modules).sum();
        assert_eq!(mods, m.modules().iter().filter(|x| x.scope.is_some()).count());
        // Sorted by path, and parents point at an existing option.
        assert!(is.list.windows(2).all(|w| w[0].path < w[1].path));
        for x in &is.list {
            let p = x.parent_option.expect("the parent option is listed");
            assert_eq!(m.options()[p].path, x.parent);
        }
    }
}

#[test]
fn a_submodule_inside_a_submodule() {
    let m = fixture();
    let is = &m.instances;
    let alice = inst(&m, "fixture.home.alice");
    let todo = inst(&m, "fixture.home.alice.files.todo");
    let notes = inst(&m, "fixture.home.alice.files.notes");
    assert_eq!(is.list[todo].enclosing, Some(alice));
    assert_eq!(is.list[todo].depth, 1);
    assert_eq!(is.list[todo].parent, "fixture.home.alice.files");
    assert_eq!(is.list[alice].children, vec![notes, todo]);
    assert_eq!(is.chain(todo), vec![alice, todo]);
    assert_eq!(is.subtree(alice), vec![alice, notes, todo]);
    assert_eq!(is.label(todo), "files.todo");
    assert_eq!(is.label(alice), "fixture.home.alice");
    let direct = is.list[alice].options.len();
    assert_eq!(
        is.list[alice].total_options,
        direct + is.list[todo].total_options + is.list[notes].total_options
    );
    assert_eq!(
        is.list[alice].total_modules,
        is.list[alice].modules.len() + is.list[todo].modules.len() + is.list[notes].modules.len()
    );
    assert_eq!(
        crumbs(&m, "fixture.home.alice.files.todo.text"),
        vec!["fixture.home", "alice", "files", "todo", "text"]
    );
    let c = m.instances.breadcrumb(
        oi(&m, "fixture.home.alice.files.todo.text"),
        "fixture.home.alice.files.todo.text",
    );
    assert_eq!(
        c[0],
        Crumb {
            text: "fixture.home".into(),
            target: CrumbTarget::Option(oi(&m, "fixture.home"))
        }
    );
    assert_eq!(c[1].target, CrumbTarget::Instance(alice));
    assert_eq!(c[2].target, CrumbTarget::Option(oi(&m, "fixture.home.alice.files")));
    assert_eq!(c[3].target, CrumbTarget::Instance(todo));
    assert_eq!(c[4].target, CrumbTarget::Here);
}

#[test]
fn a_single_submodule_has_no_name_segment() {
    let m = fixture();
    let i = inst(&m, "fixture.single");
    assert_eq!(m.instances.list[i].parent, "fixture.single");
    assert_eq!(crumbs(&m, "fixture.single.shell"), vec!["fixture.single", "shell"]);
}

#[test]
fn a_quoted_instance_name() {
    let m = demo();
    let i = inst(&m, "fileSystems.\"/boot\"");
    assert_eq!(m.instances.list[i].name, "/boot");
    assert_eq!(m.instances.list[i].parent, "fileSystems");
}

#[test]
fn instances_with_modules_only() {
    // A freeform key touched: a nested module, but no nested option.
    let m = fixture();
    let i = inst(&m, "services.openssh.settings");
    let x = &m.instances.list[i];
    assert!(x.options.is_empty());
    assert!(!x.modules.is_empty());
    assert_eq!(x.parent, "services.openssh.settings");
}

#[test]
fn finding_an_instance() {
    let m = fixture();
    let is = &m.instances;
    assert_eq!(is.find("fixture.home.alice"), Ok(inst(&m, "fixture.home.alice")));
    assert_eq!(is.find("bob"), Ok(inst(&m, "fixture.home.bob")));
    assert_eq!(is.find("nope"), Err(vec![]));
    // alice is fixture.home.alice and users.users.alice.
    let Err(paths) = is.find("alice") else {
        panic!("alice is ambiguous")
    };
    assert_eq!(paths, vec!["fixture.home.alice", "users.users.alice"]);
}

#[test]
fn dropped_definitions_say_where() {
    let m = fixture();
    // Lost at a listed option: its winning priority is known.
    let oi_ = oi(&m, "fixture.allForced.gina.shell");
    let o = &m.options()[oi_];
    let d = o.definitions.iter().position(|d| d.shadowed_at.is_some()).unwrap();
    let row = m.ladder(oi_).into_iter().find(|r| r.def == d).unwrap();
    assert_eq!(row.status, Status::Lose);
    assert_eq!(row.reason, "dropped at fixture.allForced (mkForce)");
    // Lost at an instance: the schema does not say to what priority.
    let oi_ = oi(&m, "fixture.home.carol.theme");
    let o = &m.options()[oi_];
    let d = o.definitions.iter().position(|d| d.shadowed_at.is_some()).unwrap();
    let row = m.ladder(oi_).into_iter().find(|r| r.def == d).unwrap();
    assert_eq!(row.reason, "dropped at fixture.home.carol");
}

#[test]
fn the_palette_lists_instances() {
    let m = fixture();
    let group = |q: &str| palette_search(&m, q).into_iter().find(|g| g.group == Group::Instances);
    let g = group("").expect("an empty query lists instances");
    assert_eq!(g.total, m.instances.list.len());
    assert!(g.items.iter().all(|it| it.kind == Kind::Instance));
    let g = group("carol").expect("carol matches");
    assert_eq!(g.total, 1);
    assert_eq!(m.instances.list[g.items[0].index].path, "fixture.home.carol");
    // A plain option query has no instance to show.
    assert!(group("networking.hostname").is_none());
    // Old graphs have no group at all.
    let old = Model::new(load::from_str(include_str!("data/demo.json")).unwrap());
    assert!(palette_search(&old, "").iter().all(|g| g.group != Group::Instances));
    assert!(old.instances.is_empty());
}
