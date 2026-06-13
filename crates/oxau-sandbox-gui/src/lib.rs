//! egui frontend for the Oxau sandbox.

#![allow(clippy::missing_docs_in_private_items)]

#[cfg(not(target_arch = "wasm32"))]
use std::env;
use std::sync::{Arc, Mutex};

use egui::{
    Color32, FontFamily, FontId, Galley, Margin, RichText, TextBuffer, TextEdit, TextFormat,
    TextStyle, Ui, text::LayoutJob,
};
use eguidev::{DevMcp, DevUiExt, FixtureSpec, FrameGuard, WidgetRole, WidgetValue};
#[cfg(all(not(target_arch = "wasm32"), feature = "devtools"))]
use eguidev_runtime::attach as attach_runtime;
use oxau_sandbox::{CheckResult, DEFAULT_SOURCE, Diagnostic, RunResult, Severity, check, execute};

/// Native window title.
pub const APP_TITLE: &str = "Oxau Sandbox";

/// Mutable app state shared with eguidev fixtures.
#[derive(Debug)]
pub struct AppState {
    /// Current editor buffer.
    pub source: String,
    /// Current checker result.
    pub check_result: CheckResult,
    /// Last run result.
    pub run_result: RunResult,
}

impl Default for AppState {
    fn default() -> Self {
        let source = DEFAULT_SOURCE.to_string();
        let check_result = check(&source);
        Self {
            source,
            check_result,
            run_result: RunResult::default(),
        }
    }
}

impl AppState {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn replace_with_error_sample(&mut self) {
        self.source = "fn main() {\n    let width: i64 = 6;\n    println!(height);\n}".to_string();
        self.check_result = check(&self.source);
        self.run_result = RunResult::default();
    }

    fn refresh_check(&mut self) {
        self.check_result = check(&self.source);
    }

    fn run(&mut self) {
        self.run_result = execute(&self.source);
        self.check_result = CheckResult {
            diagnostics: self.run_result.diagnostics.clone(),
        };
    }
}

/// Root eframe application.
pub struct SandboxApp {
    devmcp: DevMcp,
    state: Arc<Mutex<AppState>>,
}

impl SandboxApp {
    /// Construct a new sandbox application.
    #[must_use]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let state = Arc::new(Mutex::new(AppState::default()));
        Self {
            devmcp: build_devmcp(false, &state),
            state,
        }
    }

    /// Construct a new sandbox application with native automation enabled.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_devtools(_cc: &eframe::CreationContext<'_>, enable_mcp: bool) -> Self {
        let state = Arc::new(Mutex::new(AppState::default()));
        Self {
            devmcp: build_devmcp(enable_mcp, &state),
            state,
        }
    }

    fn show_top_bar(&self, ui: &mut Ui, state: &mut AppState) {
        eguidev::container(ui, "app.toolbar", |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new(APP_TITLE).color(theme::ACCENT));
                ui.separator();

                let status = if state.check_result.is_ok() {
                    RichText::new("checked").color(theme::GOOD)
                } else {
                    RichText::new("errors").color(theme::BAD)
                };
                ui.dev_label("status.check", status);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.dev_button("action.reset", "Reset").clicked() {
                        state.reset();
                    }
                    if ui
                        .dev_button("action.sample_error", "Error Sample")
                        .clicked()
                    {
                        state.replace_with_error_sample();
                    }
                    if ui.dev_button("action.run", "Run").clicked() {
                        state.run();
                    }
                    if ui.dev_button("action.check", "Check").clicked() {
                        state.refresh_check();
                    }
                });
            });
        });
    }

    fn show_editor(&self, ui: &mut Ui, state: &mut AppState) {
        ui.vertical(|ui| {
            ui.dev_label("editor.heading", RichText::new("Editor").strong());
            let mut layouter = |ui: &Ui, text: &dyn TextBuffer, wrap_width: f32| -> Arc<Galley> {
                let mut job = highlight_rust(text.as_str());
                job.wrap.max_width = wrap_width;
                ui.fonts_mut(|fonts| fonts.layout_job(job))
            };

            let response = eguidev::id_with_meta(
                ui,
                "editor.source",
                WidgetRole::TextEdit,
                Some("source".to_string()),
                Some(WidgetValue::Text(state.source.clone())),
                |ui| {
                    ui.add(
                        TextEdit::multiline(&mut state.source)
                            .font(TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .code_editor()
                            .layouter(&mut layouter),
                    )
                },
            );

            if response.changed() {
                state.refresh_check();
            }
        });
    }

    fn show_diagnostics(&self, ui: &mut Ui, state: &AppState) {
        ui.vertical(|ui| {
            ui.dev_label("diagnostics.heading", RichText::new("Diagnostics").strong());
            egui::ScrollArea::vertical()
                .id_salt("diagnostics.scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if state.check_result.diagnostics.is_empty() {
                        ui.dev_label(
                            "diagnostics.empty",
                            RichText::new("No errors").color(theme::GOOD),
                        );
                        return;
                    }

                    for (index, diagnostic) in state.check_result.diagnostics.iter().enumerate() {
                        show_diagnostic(ui, index, diagnostic);
                    }
                });
        });
    }

    fn show_output(&self, ui: &mut Ui, state: &AppState) {
        ui.vertical(|ui| {
            ui.dev_label("output.heading", RichText::new("Output").strong());
            let output = if !state.run_result.is_ok() {
                "Run failed. Fix diagnostics and try again.".to_string()
            } else if state.run_result.output.is_empty() {
                "Run the program to see output.".to_string()
            } else {
                state.run_result.output.clone()
            };
            eguidev::id_with_meta(
                ui,
                "output.text",
                WidgetRole::Label,
                Some("output".to_string()),
                Some(WidgetValue::Text(output.clone())),
                |ui| {
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(output)
                                        .monospace()
                                        .color(theme::TEXT)
                                        .size(13.0),
                                )
                                .wrap(),
                            )
                        })
                        .inner
                },
            );
        });
    }
}

impl eframe::App for SandboxApp {
    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {}

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let devmcp = self.devmcp.clone();
        let _guard = FrameGuard::new(&devmcp, &ctx);

        theme::apply(&ctx);

        egui::Frame::central_panel(ui.style())
            .fill(theme::BACKGROUND)
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                let mut state = self.state.lock().expect("app state lock");
                self.show_top_bar(ui, &mut state);
                ui.add_space(10.0);
                ui.columns(2, |columns| {
                    self.show_editor(&mut columns[0], &mut state);
                    columns[1].vertical(|ui| {
                        self.show_diagnostics(ui, &state);
                        ui.add_space(12.0);
                        self.show_output(ui, &state);
                    });
                });
            });
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        eguidev::raw_input_hook(&self.devmcp, ctx, raw_input);
    }
}

/// Run the native sandbox application.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> anyhow::Result<()> {
    let enable_mcp = env::args().any(|arg| arg == "--dev-mcp");
    validate_devtools(enable_mcp)?;

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_title(APP_TITLE),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(move |cc| Ok(Box::new(SandboxApp::with_devtools(cc, enable_mcp)))),
    )
    .map_err(|err| anyhow::anyhow!(err.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_devtools(enable_mcp: bool) -> anyhow::Result<()> {
    #[cfg(not(feature = "devtools"))]
    if enable_mcp {
        anyhow::bail!("--dev-mcp requires building with `--features devtools`");
    }

    let _ = enable_mcp;
    Ok(())
}

fn build_devmcp(enable_mcp: bool, state: &Arc<Mutex<AppState>>) -> DevMcp {
    let state_for_handler = Arc::clone(state);
    let devmcp = DevMcp::new()
        .fixtures(vec![
            FixtureSpec::new("sandbox.default", "Default checked source.").anchor("editor.source"),
            FixtureSpec::new("sandbox.error", "Source with one undefined value.")
                .anchor("diagnostics.0"),
        ])
        .on_fixture(move |name| {
            let mut state = state_for_handler.lock().map_err(|err| err.to_string())?;
            match name {
                "sandbox.default" => {
                    state.reset();
                    Ok(())
                }
                "sandbox.error" => {
                    state.replace_with_error_sample();
                    Ok(())
                }
                _ => Err(format!("unknown fixture: {name}")),
            }
        });

    #[cfg(all(not(target_arch = "wasm32"), feature = "devtools"))]
    if enable_mcp {
        return attach_runtime(devmcp);
    }

    let _ = enable_mcp;
    devmcp
}

fn show_diagnostic(ui: &mut Ui, index: usize, diagnostic: &Diagnostic) {
    let color = match diagnostic.severity {
        Severity::Error => theme::BAD,
        Severity::Warning => theme::WARN,
    };
    let text = format!(
        "{}:{}: {}",
        diagnostic.span.line, diagnostic.span.column, diagnostic.message
    );

    eguidev::container(ui, format!("diagnostics.{index}"), |ui| {
        ui.dev_label(
            format!("diagnostics.{index}.message"),
            RichText::new(text).color(color).monospace(),
        );
        if let Some(help) = &diagnostic.help {
            ui.dev_label(
                format!("diagnostics.{index}.help"),
                RichText::new(help).color(theme::MUTED),
            );
        }
        ui.add_space(6.0);
    });
}

fn highlight_rust(source: &str) -> LayoutJob {
    let monospace = FontId::new(13.0, FontFamily::Monospace);
    let mut job = LayoutJob::default();
    let mut rest = source;

    while !rest.is_empty() {
        if let Some(comment_start) = rest.find("//")
            && comment_start == 0
        {
            push(&mut job, rest, &monospace, theme::COMMENT);
            break;
        }

        let Some((token, kind, tail)) = next_token(rest) else {
            push(&mut job, rest, &monospace, theme::TEXT);
            break;
        };
        push(&mut job, token, &monospace, token_color(kind));
        rest = tail;
    }

    job.wrap.break_anywhere = false;
    job
}

#[derive(Debug, Clone, Copy)]
enum TokenKind {
    Plain,
    Keyword,
    Number,
    String,
    Comment,
    Macro,
}

fn next_token(input: &str) -> Option<(&str, TokenKind, &str)> {
    let first = input.chars().next()?;
    if first == '"' {
        let len = string_token_len(input);
        return Some((&input[..len], TokenKind::String, &input[len..]));
    }
    if input.starts_with("//") {
        return Some((input, TokenKind::Comment, ""));
    }
    if first.is_ascii_digit() {
        let len = take_while(input, |ch| ch.is_ascii_digit());
        return Some((&input[..len], TokenKind::Number, &input[len..]));
    }
    if first == '_' || first.is_ascii_alphabetic() {
        let len = take_while(input, |ch| ch == '_' || ch.is_ascii_alphanumeric());
        let mut token_len = len;
        let mut kind = if is_keyword(&input[..len]) {
            TokenKind::Keyword
        } else {
            TokenKind::Plain
        };
        if input[len..].starts_with('!') {
            token_len += 1;
            kind = TokenKind::Macro;
        }
        return Some((&input[..token_len], kind, &input[token_len..]));
    }

    let len = first.len_utf8();
    Some((&input[..len], TokenKind::Plain, &input[len..]))
}

fn string_token_len(input: &str) -> usize {
    let mut escaped = false;
    for (idx, ch) in input.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return idx + 1;
        }
    }
    input.len()
}

fn take_while(input: &str, pred: impl Fn(char) -> bool) -> usize {
    input
        .char_indices()
        .find_map(|(idx, ch)| (!pred(ch)).then_some(idx))
        .unwrap_or(input.len())
}

fn is_keyword(token: &str) -> bool {
    matches!(
        token,
        "as" | "break"
            | "const"
            | "continue"
            | "else"
            | "enum"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn token_color(kind: TokenKind) -> Color32 {
    match kind {
        TokenKind::Plain => theme::TEXT,
        TokenKind::Keyword => theme::KEYWORD,
        TokenKind::Number => theme::NUMBER,
        TokenKind::String => theme::STRING,
        TokenKind::Comment => theme::COMMENT,
        TokenKind::Macro => theme::MACRO,
    }
}

fn push(job: &mut LayoutJob, text: &str, font_id: &FontId, color: Color32) {
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font_id.clone(),
            color,
            ..Default::default()
        },
    );
}

mod theme {
    use egui::{Color32, Context, Stroke, Style, Visuals};

    pub const BACKGROUND: Color32 = Color32::from_rgb(0x10, 0x12, 0x18);
    pub const PANEL: Color32 = Color32::from_rgb(0x18, 0x1d, 0x24);
    pub const TEXT: Color32 = Color32::from_rgb(0xea, 0xed, 0xf3);
    pub const MUTED: Color32 = Color32::from_rgb(0xa8, 0xb1, 0xc2);
    pub const ACCENT: Color32 = Color32::from_rgb(0x67, 0xd8, 0xef);
    pub const GOOD: Color32 = Color32::from_rgb(0x6d, 0xdb, 0x8b);
    pub const WARN: Color32 = Color32::from_rgb(0xf2, 0xc4, 0x66);
    pub const BAD: Color32 = Color32::from_rgb(0xff, 0x7a, 0x7a);
    pub const KEYWORD: Color32 = Color32::from_rgb(0xc7, 0x92, 0xea);
    pub const NUMBER: Color32 = Color32::from_rgb(0xf7, 0xc8, 0x73);
    pub const STRING: Color32 = Color32::from_rgb(0x9c, 0xd6, 0x7a);
    pub const COMMENT: Color32 = Color32::from_rgb(0x7c, 0x88, 0x99);
    pub const MACRO: Color32 = Color32::from_rgb(0x82, 0xaa, 0xff);

    pub fn apply(ctx: &Context) {
        let mut style = Style {
            visuals: Visuals::dark(),
            ..(*ctx.global_style()).clone()
        };
        style.visuals.panel_fill = BACKGROUND;
        style.visuals.window_fill = PANEL;
        style.visuals.widgets.noninteractive.bg_fill = PANEL;
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(0x20, 0x25, 0x2e);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x29, 0x31, 0x3d);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(0x30, 0x3a, 0x49);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        ctx.set_global_style(style);
    }
}
