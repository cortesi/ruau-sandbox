//! egui frontend for the Ruau sandbox.

#![allow(clippy::missing_docs_in_private_items)]

use std::sync::{Arc, Mutex};
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, path::PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context;
#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;
use egui::{
    Color32, CursorIcon, FontFamily, FontId, Galley, Margin, Rect, RichText, Sense, TextBuffer,
    TextEdit, TextFormat, TextStyle, Ui, UiBuilder, text::LayoutJob,
};
use eguidev::{
    DevMcp, DevUiExt, FixtureSpec, FrameGuard, RoleState, ScrollAreaState, WidgetMeta, WidgetRole,
    WidgetValue, capture_layout, take_widget_value_override, track_response_full,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "devtools"))]
use eguidev_runtime::attach as attach_runtime;
use ruau_sandbox::{CheckResult, DEFAULT_SOURCE, Diagnostic, RunResult, Severity, check, execute};

/// Native window title.
pub const APP_TITLE: &str = "Ruau Sandbox";

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Parser)]
#[command(
    name = "ruau-sandbox-gui",
    version,
    about = "Edit, check, and run Rust-like sandbox scripts."
)]
struct NativeArgs {
    /// Enable the embedded eguidev MCP runtime.
    #[arg(long = "dev-mcp")]
    enable_mcp: bool,
    /// Script file to load into the editor on startup.
    script: Option<PathBuf>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeArgs {
    fn load_source(&self) -> anyhow::Result<Option<String>> {
        self.script
            .as_ref()
            .map(|path| {
                fs::read_to_string(path)
                    .with_context(|| format!("failed to read script {}", path.display()))
            })
            .transpose()
    }
}

/// Mutable app state shared with eguidev fixtures.
#[derive(Debug)]
pub struct AppState {
    /// Current editor buffer.
    pub source: String,
    /// Current checker result.
    pub check_result: CheckResult,
    /// Last run result.
    pub run_result: RunResult,
    run_history: Vec<RunRecord>,
    next_run_number: usize,
    editor_scroll: ScrollAreaState,
    output_scroll: ScrollAreaState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_source(DEFAULT_SOURCE.to_string())
    }
}

impl AppState {
    fn with_source(source: String) -> Self {
        let check_result = check(&source);
        Self {
            source,
            check_result,
            run_result: RunResult::default(),
            run_history: Vec::new(),
            next_run_number: 1,
            editor_scroll: ScrollAreaState::default(),
            output_scroll: ScrollAreaState::default(),
        }
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn replace_with_error_sample(&mut self) {
        self.source = "fn main() {\n    let width: i64 = 6;\n    println!(height);\n}".to_string();
        self.check_result = check(&self.source);
        self.clear_runs();
    }

    fn refresh_check(&mut self) {
        self.check_result = check(&self.source);
    }

    fn run(&mut self) {
        self.run_result = execute(&self.source);
        self.check_result = CheckResult {
            diagnostics: self.run_result.diagnostics.clone(),
        };
        self.run_history.push(RunRecord {
            number: self.next_run_number,
            result: self.run_result.clone(),
        });
        self.next_run_number += 1;
    }

    fn clear_runs(&mut self) {
        self.run_result = RunResult::default();
        self.run_history.clear();
        self.next_run_number = 1;
        self.editor_scroll.reset();
        self.output_scroll.reset();
    }

    fn output_history_text(&self) -> String {
        if self.run_history.is_empty() {
            return "Run the program to see output.".to_string();
        }

        self.run_history
            .iter()
            .map(RunRecord::display_text)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// A rendered script execution kept in the output history.
#[derive(Debug, Clone)]
struct RunRecord {
    number: usize,
    result: RunResult,
}

impl RunRecord {
    fn display_text(&self) -> String {
        let mut text = format!(
            "Run {} - {}",
            self.number,
            if self.result.is_ok() { "ok" } else { "failed" }
        );

        if self.result.output.is_empty() {
            if self.result.is_ok() {
                text.push_str("\nProgram produced no output.");
            } else {
                text.push_str("\nRun failed. Fix diagnostics and try again.");
            }
        } else {
            text.push('\n');
            text.push_str(self.result.output.trim_end());
        }

        if !self.result.diagnostics.is_empty() {
            text.push_str("\nDiagnostics:");
            for diagnostic in &self.result.diagnostics {
                text.push_str(&format!(
                    "\n{}:{}: {}",
                    diagnostic.span.line, diagnostic.span.column, diagnostic.message
                ));
                if let Some(help) = &diagnostic.help {
                    text.push_str(&format!("\n  help: {help}"));
                }
            }
        }

        text
    }
}

/// Root eframe application.
pub struct SandboxApp {
    devmcp: DevMcp,
    state: Arc<Mutex<AppState>>,
    split_ratio: f32,
    split_drag_start_width: Option<f32>,
}

impl SandboxApp {
    /// Construct a new sandbox application.
    #[must_use]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let state = Arc::new(Mutex::new(AppState::default()));
        Self {
            devmcp: build_devmcp(false, &state),
            state,
            split_ratio: DEFAULT_SPLIT_RATIO,
            split_drag_start_width: None,
        }
    }

    /// Construct a new sandbox application with native automation enabled.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn with_devtools(
        _cc: &eframe::CreationContext<'_>,
        enable_mcp: bool,
        initial_source: String,
    ) -> Self {
        let state = Arc::new(Mutex::new(AppState::with_source(initial_source)));
        Self {
            devmcp: build_devmcp(enable_mcp, &state),
            state,
            split_ratio: DEFAULT_SPLIT_RATIO,
            split_drag_start_width: None,
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
            let mut source_changed = false;
            if let Some(WidgetValue::Text(source)) = take_widget_value_override(ui, "editor.source")
            {
                state.source = source;
                source_changed = true;
            }

            let mut layouter = |ui: &Ui, text: &dyn TextBuffer, wrap_width: f32| -> Arc<Galley> {
                let mut job = highlight_rust(text.as_str());
                job.wrap.max_width = wrap_width;
                ui.fonts_mut(|fonts| fonts.layout_job(job))
            };

            let editor_height = ui.available_height().max(0.0);
            let response = state
                .editor_scroll
                .show_viewport(
                    egui::ScrollArea::vertical()
                        .id_salt("editor.scroll")
                        .auto_shrink([false, false])
                        .max_height(editor_height)
                        .min_scrolled_height(editor_height),
                    ui,
                    "editor.scroll",
                    |ui, viewport| {
                        ui.set_min_width(ui.available_width());
                        let response = ui.add_sized(
                            [ui.available_width(), viewport.height()],
                            TextEdit::multiline(&mut state.source)
                                .font(TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(1)
                                .code_editor()
                                .layouter(&mut layouter),
                        );
                        track_response_full(
                            "editor.source",
                            &response,
                            WidgetMeta {
                                role: WidgetRole::TextEdit,
                                label: Some("source".to_string()),
                                value: Some(WidgetValue::Text(state.source.clone())),
                                role_state: Some(RoleState::TextEdit {
                                    multiline: true,
                                    password: false,
                                }),
                                layout: Some(capture_layout(ui, &response)),
                                visible: ui.is_visible() && ui.is_rect_visible(response.rect),
                                ..WidgetMeta::default()
                            },
                        );
                        response
                    },
                )
                .inner;

            if source_changed || response.changed() {
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

    fn show_output(&self, ui: &mut Ui, state: &mut AppState) {
        ui.vertical(|ui| {
            ui.dev_label("output.heading", RichText::new("Output").strong());
            ui.add_space(6.0);

            let body_size = ui.available_size();
            ui.allocate_ui(body_size, |ui| {
                eguidev::container(ui, "output.panel", |ui| {
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            let output = state.output_history_text();
                            show_output_history(ui, state, output);
                        });
                });
            });
        });
    }

    fn show_results_column(&self, ui: &mut Ui, state: &mut AppState) {
        let gap = 12.0;
        let available_height = ui.available_height().max(0.0);
        let output_min_height = 130.0;
        let diagnostics_min_height = 150.0;
        let diagnostics_height =
            if available_height > diagnostics_min_height + output_min_height + gap {
                (available_height * 0.58).clamp(
                    diagnostics_min_height,
                    available_height - output_min_height - gap,
                )
            } else {
                ((available_height - gap).max(0.0)) * 0.5
            };

        ui.allocate_ui(egui::vec2(ui.available_width(), diagnostics_height), |ui| {
            self.show_diagnostics(ui, state);
        });
        ui.add_space(gap);
        ui.allocate_ui(
            egui::vec2(ui.available_width(), ui.available_height()),
            |ui| {
                self.show_output(ui, state);
            },
        );
    }

    fn show_workspace(&mut self, ui: &mut Ui, state: &mut AppState) {
        let (_id, rect) = ui.allocate_space(ui.available_size());
        if rect.width() <= 1.0 || rect.height() <= 1.0 {
            return;
        }

        let layout = SplitLayout::new(rect, self.split_ratio);
        self.show_split_divider(ui, &layout);

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(layout.left_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                eguidev::container(ui, "layout.left_pane", |ui| {
                    self.show_editor(ui, state);
                });
            },
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(layout.right_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                eguidev::container(ui, "layout.right_pane", |ui| {
                    self.show_results_column(ui, state);
                });
            },
        );
    }

    fn show_split_divider(&mut self, ui: &Ui, layout: &SplitLayout) {
        let response = ui
            .interact(
                layout.divider_rect,
                ui.id().with("layout.divider"),
                Sense::drag(),
            )
            .on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);
        if response.drag_started() {
            self.split_drag_start_width = Some(layout.left_width);
        }
        if response.dragged() {
            let start_width = self.split_drag_start_width.unwrap_or(layout.left_width);
            let delta_x = response
                .total_drag_delta()
                .unwrap_or_else(|| response.drag_delta())
                .x;
            self.split_ratio = layout.ratio_for_left_width(start_width + delta_x);
            ui.ctx().request_repaint();
        }
        if response.drag_stopped() {
            self.split_drag_start_width = None;
        }

        let color = if response.dragged() || response.hovered() {
            theme::ACCENT
        } else {
            theme::DIVIDER
        };
        let line = Rect::from_center_size(
            layout.divider_rect.center(),
            egui::vec2(1.0, layout.divider_rect.height()),
        );
        ui.painter()
            .rect_filled(layout.divider_rect, 0.0, theme::BACKGROUND);
        ui.painter()
            .rect_filled(line, 0.0, color.gamma_multiply(0.8));
        track_response_full(
            "layout.divider",
            &response,
            WidgetMeta {
                role: WidgetRole::Separator,
                label: Some("pane divider".to_string()),
                layout: Some(capture_layout(ui, &response)),
                visible: ui.is_visible() && ui.is_rect_visible(response.rect),
                ..WidgetMeta::default()
            },
        );
    }
}

const DEFAULT_SPLIT_RATIO: f32 = 0.5;
const DIVIDER_WIDTH: f32 = 8.0;
const MIN_LEFT_PANE_WIDTH: f32 = 260.0;
const MIN_RIGHT_PANE_WIDTH: f32 = 300.0;

#[derive(Debug, Clone, Copy)]
struct SplitLayout {
    left_rect: Rect,
    divider_rect: Rect,
    right_rect: Rect,
    content_width: f32,
    left_width: f32,
    min_left: f32,
    max_left: f32,
}

impl SplitLayout {
    fn new(rect: Rect, ratio: f32) -> Self {
        let divider_width = DIVIDER_WIDTH.min(rect.width()).max(0.0);
        let content_width = (rect.width() - divider_width).max(0.0);
        let min_left = MIN_LEFT_PANE_WIDTH.min(content_width * 0.5);
        let min_right = MIN_RIGHT_PANE_WIDTH.min((content_width - min_left).max(0.0));
        let max_left = (content_width - min_right).max(min_left);
        let left_width = (content_width * ratio.clamp(0.0, 1.0)).clamp(min_left, max_left);
        let divider_min_x = rect.min.x + left_width;
        let divider_max_x = divider_min_x + divider_width;
        let left_rect = Rect::from_min_max(rect.min, egui::pos2(divider_min_x, rect.max.y));
        let divider_rect = Rect::from_min_max(
            egui::pos2(divider_min_x, rect.min.y),
            egui::pos2(divider_max_x, rect.max.y),
        );
        let right_rect = Rect::from_min_max(egui::pos2(divider_max_x, rect.min.y), rect.max);

        Self {
            left_rect,
            divider_rect,
            right_rect,
            content_width,
            left_width,
            min_left,
            max_left,
        }
    }

    fn ratio_for_left_width(self, left_width: f32) -> f32 {
        if self.content_width <= 0.0 {
            return DEFAULT_SPLIT_RATIO;
        }
        (left_width.clamp(self.min_left, self.max_left) / self.content_width).clamp(0.0, 1.0)
    }
}

fn show_output_history(ui: &mut Ui, state: &mut AppState, output: String) {
    let scroll_height = ui.available_height().max(0.0);
    state.output_scroll.show(
        egui::ScrollArea::vertical()
            .id_salt("output.history.scroll")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height(scroll_height)
            .min_scrolled_height(scroll_height),
        ui,
        "output.history",
        |ui| {
            ui.set_min_width(ui.available_width());
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                show_output_text(ui, output);
            });
        },
    );
}

fn show_output_text(ui: &mut Ui, output: String) -> egui::Response {
    eguidev::id_with_meta(
        ui,
        "output.text",
        WidgetRole::Label,
        Some("output".to_string()),
        Some(WidgetValue::Text(output.clone())),
        |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(output)
                        .monospace()
                        .color(theme::TEXT)
                        .size(13.0),
                )
                .wrap(),
            )
        },
    )
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
                let state = Arc::clone(&self.state);
                let mut state = state.lock().expect("app state lock");
                self.show_top_bar(ui, &mut state);
                ui.add_space(10.0);
                self.show_workspace(ui, &mut state);
            });
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        eguidev::raw_input_hook(&self.devmcp, ctx, raw_input);
    }
}

/// Run the native sandbox application.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() -> anyhow::Result<()> {
    let args = NativeArgs::parse();
    let enable_mcp = args.enable_mcp;
    validate_devtools(enable_mcp)?;
    let initial_source = args
        .load_source()?
        .unwrap_or_else(|| DEFAULT_SOURCE.to_string());

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
        Box::new(move |cc| {
            Ok(Box::new(SandboxApp::with_devtools(
                cc,
                enable_mcp,
                initial_source,
            )))
        }),
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
    pub const DIVIDER: Color32 = Color32::from_rgb(0x3a, 0x44, 0x52);
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

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use std::{env, fs, path::Path, process};

    #[cfg(not(target_arch = "wasm32"))]
    use clap::Parser;

    #[cfg(not(target_arch = "wasm32"))]
    use super::NativeArgs;
    use super::{AppState, highlight_rust, theme};

    #[test]
    fn syntax_highlighting_classifies_rust_tokens() {
        let job =
            highlight_rust("fn main() {\n    let n = 42;\n    println!(\"{n}\"); // output\n}");
        let colors = job
            .sections
            .iter()
            .map(|section| section.format.color)
            .collect::<Vec<_>>();

        assert!(colors.contains(&theme::KEYWORD));
        assert!(colors.contains(&theme::NUMBER));
        assert!(colors.contains(&theme::STRING));
        assert!(colors.contains(&theme::MACRO));
        assert!(colors.contains(&theme::COMMENT));
    }

    #[test]
    fn app_state_checks_initial_source() {
        let state = AppState::with_source("fn main() {\n    println!(missing);\n}".to_string());

        assert!(!state.check_result.is_ok());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_args_parse_optional_script() {
        let args = NativeArgs::parse_from(["ruau-sandbox-gui", "--dev-mcp", "sample.rs"]);

        assert!(args.enable_mcp);
        assert_eq!(args.script.as_deref(), Some(Path::new("sample.rs")));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_args_load_source_from_script() -> anyhow::Result<()> {
        let source = "fn main() {\n    println!(\"loaded\");\n}\n";
        let path = env::temp_dir().join(format!("ruau-sandbox-script-load-{}.rs", process::id()));
        fs::write(&path, source)?;

        let args = NativeArgs {
            enable_mcp: false,
            script: Some(path.clone()),
        };
        let loaded_source = args.load_source()?;
        fs::remove_file(path)?;

        assert_eq!(loaded_source.as_deref(), Some(source));
        Ok(())
    }
}
