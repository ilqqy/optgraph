//! optgraph-app: the native viewer of optgraph's graph.json.
//!
//!   optgraph-app graph.json [--opt <option path>] [--module <index>]
//!   optgraph-app - < graph.json

mod assets;
mod evaluate;
mod graph;
mod input;
mod inspector;
mod loading;
mod palette;
mod sidebar;
mod stage;
mod theme;
mod ui;
mod view;

use std::path::PathBuf;
use std::process::ExitCode;

use gpui::{App, AppContext, Bounds, Focusable, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use optgraph_core::{Model, load};

use crate::theme::Theme;
use crate::view::{OptgraphView, Sel};

const USAGE: &str = "usage: optgraph-app [graph.json | - | <flakeref>#nixosConfigurations.<host>]
                    [--opt <option path>] [--module <index>] [--light | --dark]

Shows who set each option of a NixOS configuration, at what priority, and
why the other definitions lost. graph.json is made by optgraph; - reads it
from stdin. A flake reference runs optgraph on it first ($OPTGRAPH_CLI, else
optgraph on PATH) and shows its progress.

  --opt <path>      select an option, e.g. networking.firewall.enable
  --module <index>  select a module by its index in graph.json's modules
  --light, --dark   colour scheme (default: dark, light when the system
                    explicitly prefers light)

Keys: / or Ctrl+K search, arrows walk the lists, Enter opens, Esc clears,
Ctrl+O opens a graph.json (or drop one on the window).";

struct Args {
    source: Option<PathBuf>,
    opt: Option<String>,
    module: Option<usize>,
    scheme: Option<theme::Mode>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        source: None,
        opt: None,
        module: None,
        scheme: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => return Err(String::new()),
            "--opt" => args.opt = Some(it.next().ok_or("--opt needs an option path")?),
            "--module" => {
                let v = it.next().ok_or("--module needs an index")?;
                args.module = Some(v.parse().map_err(|_| format!("--module: not an index: {v}"))?);
            }
            "--light" => args.scheme = Some(theme::Mode::Light),
            "--dark" => args.scheme = Some(theme::Mode::Dark),
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s => {
                if args.source.replace(PathBuf::from(s)).is_some() {
                    return Err("only one graph.json".into());
                }
            }
        }
    }
    Ok(args)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) if e.is_empty() => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("optgraph-app: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let flake_ref = args
        .source
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| evaluate::is_flake_ref(s));
    let (model, load_error) = match &args.source {
        None => (None, None),
        Some(_) if flake_ref.is_some() => (None, None),
        Some(p) => match load::from_path(p) {
            Ok(g) => (Some(Model::new(g)), None),
            Err(e) => {
                eprintln!("optgraph-app: {}: {e}", p.display());
                (None, Some(format!("{}: {e}", p.display())))
            }
        },
    };
    if let (Some(m), Some(path)) = (&model, &args.opt)
        && m.option_index(path).is_none()
    {
        eprintln!("optgraph-app: no option {path} in this graph");
        return ExitCode::from(2);
    }
    if let (Some(m), Some(i)) = (&model, args.module)
        && i >= m.modules().len()
    {
        eprintln!("optgraph-app: no module #{i} (the graph has {})", m.modules().len());
        return ExitCode::from(2);
    }
    let source = args
        .source
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            if let Err(e) = cx.text_system().add_fonts(assets::fonts()) {
                eprintln!("optgraph-app: loading the fonts failed: {e:#}");
            }
            cx.set_global(Theme::for_mode(args.scheme.unwrap_or(theme::Mode::Dark)));
            cx.bind_keys(input::bindings());
            cx.bind_keys(view::bindings());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let host = model.as_ref().and_then(|m| m.graph.meta.host.clone());
            let title = match host {
                Some(h) => format!("optgraph: {h}"),
                None => "optgraph".to_string(),
            };
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let opt = args.opt.clone();
            let module = args.module;
            let scheme = args.scheme;
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some(title.into()),
                            ..Default::default()
                        }),
                        app_id: Some("optgraph-app".into()),
                        window_min_size: Some(size(px(900.), px(560.))),
                        ..Default::default()
                    },
                    |_, cx| {
                        cx.new(|cx| {
                            let mut v = OptgraphView::new(model, source, cx);
                            v.load_error = load_error;
                            v
                        })
                    },
                )
                .expect("open the window");
            window
                .update(cx, |view, window, cx| {
                    window.focus(&view.focus_handle(cx), cx);
                    if let Some(m) = scheme {
                        view.scheme = loading::Scheme::Pinned(m);
                    }
                    view.watch_appearance(window, cx);
                    if let Some(target) = flake_ref.clone() {
                        view.after_load = (opt.clone(), module);
                        view.start_evaluation(target, window, cx);
                    }
                    if let Some(p) = &opt {
                        view.select_option_path(p, cx);
                    } else if let Some(i) = module {
                        view.select(Sel::Module(i), cx);
                    }
                })
                .ok();
            if std::env::var_os("OPTGRAPH_DEBUG_KEYS").is_some() {
                cx.observe_keystrokes(|ev, _, _| eprintln!("optgraph-app: key {:?}", ev.keystroke))
                    .detach();
            }
            cx.activate(true);
        });
    ExitCode::SUCCESS
}
