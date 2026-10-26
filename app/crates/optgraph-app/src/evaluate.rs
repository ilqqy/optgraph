//! `optgraph-app <flakeref>#nixosConfigurations.<host>`: runs the optgraph
//! CLI ($OPTGRAPH_CLI, else `optgraph` on PATH) on a background thread,
//! shows its progress lines while it evaluates, then the graph it wrote.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use gpui::{AnyElement, Context, FontWeight, IntoElement, Window, div, prelude::*, px};
use optgraph_core::{Model, load};

use crate::sidebar::logo;
use crate::theme::{FONT_MONO, FS_LG, FS_SM, FS_XS, R_MD, R_XL, Theme};
use crate::ui::shadow_3;
use crate::view::OptgraphView;

/// A flake reference rather than a file: has a `#` and is no existing path.
pub fn is_flake_ref(arg: &str) -> bool {
    arg.contains('#') && !Path::new(arg).exists()
}

pub struct Evaluation {
    pub target: String,
    pub lines: Vec<String>,
    pub started: Instant,
    /// Set when the CLI could not run or failed: what to show.
    pub failed: Option<String>,
    /// How long the CLI ran, once it has stopped.
    pub took: Option<Duration>,
    output: PathBuf,
}

enum Msg {
    Line(String),
    Exit(Option<i32>),
    Failed(String),
}

fn cli() -> String {
    std::env::var("OPTGRAPH_CLI").unwrap_or_else(|_| "optgraph".into())
}

impl OptgraphView {
    pub fn start_evaluation(&mut self, target: String, window: &mut Window, cx: &mut Context<Self>) {
        let output = std::env::temp_dir().join(format!("optgraph-app-{}.json", std::process::id()));
        let (tx, rx) = mpsc::channel::<Msg>();
        let (cli, t, out) = (cli(), target.clone(), output.clone());
        std::thread::spawn(move || {
            let child = Command::new(&cli)
                .arg(&t)
                .arg("-o")
                .arg(&out)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Msg::Failed(format!(
                        "cannot run the optgraph CLI ({cli}): {e}. Run the app with `nix run .#app`, or set OPTGRAPH_CLI."
                    )));
                    return;
                }
            };
            if let Some(err) = child.stderr.take() {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    if tx.send(Msg::Line(line)).is_err() {
                        break;
                    }
                }
            }
            let code = child.wait().ok().and_then(|s| s.code());
            let _ = tx.send(Msg::Exit(code));
        });
        self.evaluation = Some(Evaluation {
            target,
            lines: Vec::new(),
            started: Instant::now(),
            failed: None,
            took: None,
            output,
        });
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let msgs: Vec<Msg> = rx.try_iter().collect();
                let done = this.update_in(cx, |this, window, cx| this.evaluation_messages(msgs, window, cx));
                if done.unwrap_or(true) {
                    break;
                }
            }
        })
        .detach();
    }

    /// Applies the CLI's messages; true when it has finished.
    fn evaluation_messages(&mut self, msgs: Vec<Msg>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(ev) = &mut self.evaluation else { return true };
        let mut exit = None;
        for m in msgs {
            match m {
                Msg::Line(l) => ev.lines.push(l),
                Msg::Failed(e) => {
                    ev.failed = Some(e);
                    exit = Some(None);
                }
                Msg::Exit(code) => exit = Some(code),
            }
        }
        cx.notify(); // the elapsed time ticks
        let Some(code) = exit else { return false };
        ev.took = Some(ev.started.elapsed());
        if ev.failed.is_some() {
            return true;
        }
        // 0: done; 3: partial graph (meta.complete = false) still written.
        if matches!(code, Some(0) | Some(3)) {
            let out = ev.output.clone();
            match load::from_path(&out) {
                Ok(g) => {
                    let _ = std::fs::remove_file(&out);
                    let host = g.meta.host.clone();
                    self.source = ev.target.clone();
                    self.evaluation = None;
                    self.set_model(Model::new(g), cx);
                    match std::mem::take(&mut self.after_load) {
                        (Some(p), _) => {
                            if !self.select_option_path(&p, cx) {
                                self.load_error = Some(format!("no option {p} in this graph"));
                            }
                        }
                        (None, Some(i)) if i < self.model.as_ref().map_or(0, |m| m.modules().len()) => {
                            self.select(crate::view::Sel::Module(i), cx)
                        }
                        _ => {}
                    }
                    window.set_window_title(&format!("optgraph: {}", host.unwrap_or_else(|| "graph".into())));
                }
                Err(e) => ev.failed = Some(format!("the CLI finished, but its output did not load: {e}")),
            }
        } else {
            let why = match code {
                Some(1) => "usage error",
                Some(2) => "the flake or its evaluation failed",
                Some(c) => return self.fail(format!("optgraph exited with code {c}")),
                None => "optgraph was killed",
            };
            ev.failed = Some(why.into());
        }
        true
    }

    fn fail(&mut self, msg: String) -> bool {
        if let Some(ev) = &mut self.evaluation {
            ev.failed = Some(msg);
        }
        true
    }

    /// While the CLI runs: its progress lines; when it failed: the error.
    pub fn render_evaluation(&mut self, t: &Theme, ev: &Evaluation) -> AnyElement {
        let secs = ev.took.unwrap_or_else(|| ev.started.elapsed()).as_secs();
        let running = ev.failed.is_none();
        let dots = ".".repeat(1 + (ev.started.elapsed().as_millis() / 400 % 3) as usize);
        let shown: Vec<&String> = ev
            .lines
            .iter()
            .rev()
            .take(14)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let lines = shown.into_iter().map(|l| {
            let (text, err) = match l.strip_prefix("optgraph: ") {
                Some(rest) => (rest.to_string(), rest.starts_with("error:")),
                None => (l.clone(), l.starts_with("error")),
            };
            div()
                .text_color(if err { t.err } else { t.text2 })
                .font_family(FONT_MONO)
                .text_size(px(FS_XS))
                .child(text)
        });
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(16.))
            .child(
                div()
                    .w(px(720.))
                    .max_w_full()
                    .p(px(28.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .rounded(px(R_XL))
                    .border_1()
                    .border_color(if running { t.line2 } else { t.err.opacity(0.5) })
                    .bg(t.s1)
                    .shadow(shadow_3(t))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(logo(t, 1.))
                            .child(
                                div()
                                    .text_size(px(FS_LG))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if running {
                                        format!("Evaluating{dots}")
                                    } else {
                                        "Evaluation failed".to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .ml_auto()
                                    .text_color(t.text3)
                                    .font_family(FONT_MONO)
                                    .text_size(px(FS_SM))
                                    .child(format!("{secs} s")),
                            ),
                    )
                    .child(
                        div()
                            .text_color(t.text)
                            .font_family(FONT_MONO)
                            .text_size(px(FS_SM))
                            .child(ev.target.clone()),
                    )
                    .when_some(ev.failed.clone(), |d, f| d.child(div().text_color(t.err).child(f)))
                    .child(
                        div()
                            .mt(px(4.))
                            .p(px(10.))
                            .min_h(px(120.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .rounded(px(R_MD))
                            .border_1()
                            .border_color(t.line)
                            .bg(t.bg)
                            .children(lines)
                            .when(ev.lines.is_empty(), |d| {
                                d.child(
                                    div()
                                        .text_color(t.text3)
                                        .text_size(px(FS_XS))
                                        .child("starting optgraph…"),
                                )
                            }),
                    )
                    .child(div().text_color(t.text3).text_size(px(FS_XS)).child(
                        "optgraph evaluates the configuration with nix; large configurations take a minute or more.",
                    )),
            )
            .into_any_element()
    }
}
