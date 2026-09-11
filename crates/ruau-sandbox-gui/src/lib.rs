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
    CursorIcon, Galley, Key, KeyboardShortcut, Margin, Modifiers, Rect, RichText, Sense, Stroke,
    TextBuffer, TextEdit, TextStyle, Ui, UiBuilder, text_edit::TextEditOutput,
};
use eguicode::highlight::Language;
use eguidev::{
    DevMcp, DevUiExt, DiagnosticError, FixtureError, FixtureResponse, FixtureSpec, FrameGuard,
    WidgetMeta, WidgetRole, WidgetRoleMeta, WidgetValue, capture_layout,
    take_widget_value_override, track_response,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "devtools"))]
use eguidev_runtime::attach as attach_runtime;
#[cfg(not(target_arch = "wasm32"))]
use ruau_sandbox::DEFAULT_SOURCE;
use ruau_sandbox::{Diagnostic, SAMPLES, Severity, sample_by_name};
use serde_json::{Value, json};

mod highlight;
mod state;
mod theme;

pub use state::AppState;
use state::RunRecord;

/// Keyboard shortcut that runs the current program.
const RUN_SHORTCUT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter);

/// Native window title.
pub const APP_TITLE: &str = "Ruau Sandbox";

const RUAU_REPO_URL: &str = "https://github.com/cortesi/ruau";
const DEFAULT_SPLIT_RATIO: f32 = 0.5;
const DIVIDER_WIDTH: f32 = 8.0;
const MIN_LEFT_PANE_WIDTH: f32 = 260.0;
const MIN_RIGHT_PANE_WIDTH: f32 = 300.0;
const GUTTER_PADDING: f32 = 10.0;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Parser)]
#[command(
    name = "ruau-sandbox-gui",
    version,
    about = "Edit, check, and run Luau sandbox scripts."
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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
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
        cc: &eframe::CreationContext<'_>,
        enable_mcp: bool,
        initial_source: String,
    ) -> Self {
        theme::install_fonts(&cc.egui_ctx);
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
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.scope(|ui| {
                        ui.visuals_mut().hyperlink_color = theme::ACCENT;
                        ui.dev_hyperlink_to(
                            "app.title.ruau",
                            RichText::new("Ruau").strong().size(19.0),
                            RUAU_REPO_URL,
                        );
                    });
                    ui.heading(RichText::new("Sandbox").color(theme::ACCENT));
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let run_hint = format!(
                        "Run the program ({})",
                        ui.ctx().format_shortcut(&RUN_SHORTCUT)
                    );
                    if ui
                        .dev_button("action.run", RichText::new("▶ Run").strong())
                        .on_hover_text(run_hint)
                        .clicked()
                    {
                        state.run();
                    }
                    ui.dev_menu_button("samples.menu", "Samples", |ui| {
                        show_samples_menu(ui, state);
                    });
                    if ui
                        .dev_button("action.reset", "Reset")
                        .on_hover_text("Restore the default program and clear output")
                        .clicked()
                    {
                        state.reset();
                    }
                });
            });
        });
    }

    fn show_editor(&self, ui: &mut Ui, state: &mut AppState) {
        ui.vertical(|ui| {
            let mut source_changed = false;
            if let Some(WidgetValue::Text(source)) = take_widget_value_override(ui, "editor.source")
            {
                state.source = source;
                source_changed = true;
            }

            let code_theme = highlight::code_theme();
            let marks = highlight::diagnostics_to_marks(&state.check_result.diagnostics);
            let mut layouter = eguicode::layouter(&code_theme, Language::Luau, &marks);

            let line_count = state.source.lines().count().max(1);
            let editor_height = ui.available_height().max(0.0);
            let AppState {
                source,
                editor_scroll,
                ..
            } = state;
            let response = editor_scroll
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
                        show_editor_text(
                            ui,
                            source,
                            viewport,
                            &code_theme,
                            &marks,
                            line_count,
                            &mut layouter,
                        )
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
            show_pane_heading(
                ui,
                "diagnostics.header",
                "diagnostics.heading",
                "Diagnostics",
                |_| {},
            );
            ui.add_space(6.0);
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
            show_pane_heading(ui, "output.header", "output.heading", "Output", |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(state.has_runs(), |ui| {
                        if ui
                            .dev_button("output.clear", "Clear")
                            .on_hover_text("Discard the run history")
                            .clicked()
                        {
                            state.clear_output();
                        }
                    });
                });
            });
            ui.add_space(6.0);

            let body_size = ui.available_size();
            ui.allocate_ui(body_size, |ui| {
                eguidev::container(ui, "output.panel", |ui| {
                    egui::Frame::new()
                        .fill(theme::PANEL)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            show_output_history(ui, state);
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
        track_response(
            "layout.divider",
            &response,
            WidgetMeta {
                role: WidgetRoleMeta::Plain(WidgetRole::Separator),
                label: Some("pane divider".to_string()),
                layout: Some(capture_layout(ui, &response)),
                visible: ui.is_visible() && ui.is_rect_visible(response.rect),
                ..WidgetMeta::default()
            },
        );
    }
}

fn show_pane_heading(
    ui: &mut Ui,
    container_id: &str,
    label_id: &str,
    title: &str,
    actions: impl FnOnce(&mut Ui),
) {
    eguidev::container(ui, container_id, |ui| {
        egui::Frame::new()
            .fill(theme::HEADER)
            .stroke(Stroke::new(1.0, theme::DIVIDER.gamma_multiply(0.7)))
            .corner_radius(4)
            .inner_margin(Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.dev_label(
                        label_id,
                        RichText::new(title).strong().color(theme::TEXT).size(13.0),
                    );
                    actions(ui);
                });
            });
    });
}

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

fn show_output_history(ui: &mut Ui, state: &mut AppState) {
    let scroll_height = ui.available_height().max(0.0);
    let output = state.output_history_text();
    let runs = state.run_history.clone();
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
                show_output_text(ui, &runs, &output);
            });
        },
    );
}

fn show_output_text(ui: &mut Ui, runs: &[RunRecord], output: &str) -> egui::Response {
    eguidev::track_widget_with_meta(
        ui,
        "output.text",
        WidgetRoleMeta::Plain(WidgetRole::Label),
        Some("output".to_string()),
        Some(WidgetValue::Text(output.to_string())),
        |ui| {
            ui.vertical(|ui| {
                ui.set_min_width(ui.available_width());
                if runs.is_empty() {
                    ui.add(
                        egui::Label::new(RichText::new(output).color(theme::MUTED).size(13.0))
                            .wrap(),
                    );
                    return;
                }

                for (index, run) in runs.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(12.0);
                    }
                    ui.add(
                        egui::Label::new(
                            RichText::new(run.header_text())
                                .strong()
                                .color(run_header_color(run))
                                .size(13.0),
                        )
                        .wrap(),
                    );
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(run.body_text())
                                .monospace()
                                .color(theme::TEXT)
                                .size(13.0),
                        )
                        .wrap(),
                    );
                }
            })
            .response
        },
    )
}

fn run_header_color(run: &RunRecord) -> egui::Color32 {
    if !run.is_ok() {
        theme::BAD
    } else if run.has_output() {
        theme::GOOD
    } else {
        theme::WARN
    }
}

impl eframe::App for SandboxApp {
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
                if ctx.input_mut(|input| input.consume_shortcut(&RUN_SHORTCUT)) {
                    state.run();
                }
                self.show_top_bar(ui, &mut state);
                ui.add_space(10.0);
                self.show_workspace(ui, &mut state);
            });
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
    let state_for_diagnostic = Arc::clone(state);
    let devmcp = DevMcp::new()
        .fixtures(vec![
            FixtureSpec::new("sandbox.default", "Default checked source.").ready("editor.source"),
            FixtureSpec::new("sandbox.error", "Broken sample with undefined values.")
                .ready("diagnostics.0"),
        ])
        .on_fixture_runtime(move |call| {
            let mut state = state_for_handler
                .lock()
                .map_err(|err| FixtureError::new("state_lock", err.to_string()))?;
            match call.name.as_str() {
                "sandbox.default" => {
                    state.reset();
                    Ok(FixtureResponse::new())
                }
                "sandbox.error" => {
                    let sample = sample_by_name("Broken program").ok_or_else(|| {
                        FixtureError::new("sandbox_fixture", "missing broken sample")
                    })?;
                    state.load_sample(sample);
                    Ok(FixtureResponse::new())
                }
                _ => Err(FixtureError::new(
                    "sandbox_fixture",
                    format!("unknown fixture: {}", call.name),
                )),
            }
        })
        .expect("hard-coded sandbox fixture handler is unique")
        .diagnostic("sandbox.state", move || {
            let state = state_for_diagnostic
                .lock()
                .map_err(|err| DiagnosticError::new("state_lock", err.to_string()))?;
            Ok(sandbox_diagnostic(&state))
        })
        .expect("hard-coded sandbox diagnostic name is unique");

    #[cfg(all(not(target_arch = "wasm32"), feature = "devtools"))]
    if enable_mcp {
        return attach_runtime(devmcp);
    }

    let _ = enable_mcp;
    devmcp
}

/// Return semantic sandbox state for smoke tests and failure bundles.
fn sandbox_diagnostic(state: &AppState) -> Value {
    json!({
        "source": {
            "bytes": state.source.len(),
            "lines": state.source.lines().count(),
        },
        "check": {
            "ok": state.check_result.is_ok(),
            "diagnostics": state.check_result.diagnostics.len(),
            "errors": count_diagnostic_severity(&state.check_result.diagnostics, Severity::Error),
            "warnings": count_diagnostic_severity(&state.check_result.diagnostics, Severity::Warning),
        },
        "run": {
            "ok": state.run_result.is_ok(),
            "diagnostics": state.run_result.diagnostics.len(),
            "errors": count_diagnostic_severity(&state.run_result.diagnostics, Severity::Error),
            "warnings": count_diagnostic_severity(&state.run_result.diagnostics, Severity::Warning),
            "output_bytes": state.run_result.output.len(),
            "has_output": !state.run_result.output.is_empty(),
        },
        "history": {
            "runs": state.run_history.len(),
            "next_run_number": state.next_run_number,
        },
    })
}

/// Count diagnostics with a specific severity.
fn count_diagnostic_severity(diagnostics: &[Diagnostic], severity: Severity) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == severity)
        .count()
}

/// Render the source text editor with a line-number gutter inside the editor
/// scroll viewport.
fn show_editor_text(
    ui: &mut Ui,
    source: &mut String,
    viewport: Rect,
    code_theme: &eguicode::CodeTheme,
    marks: &eguicode::Marks,
    line_count: usize,
    layouter: &mut dyn FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<Galley>,
) -> egui::Response {
    let digits = line_count.max(10).to_string().len();
    let glyph_width = ui.fonts_mut(|fonts| fonts.glyph_width(&code_theme.gutter_font, '0'));
    let gutter_width = (digits as f32).mul_add(glyph_width, GUTTER_PADDING * 2.0);
    // egui's `Margin` fields are `i8`; round up so the gutter is never too
    // narrow, then reuse the rounded value for the chrome painted below so
    // the fill and divider line up with where the margin actually put the
    // text.
    let gutter_margin = gutter_width.ceil().clamp(0.0, 127.0) as i8;
    let gutter_width = f32::from(gutter_margin);

    let output = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), viewport.height()),
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                TextEdit::multiline(source)
                    .font(TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(1)
                    .code_editor()
                    .margin(Margin {
                        left: gutter_margin,
                        right: 4,
                        top: 2,
                        bottom: 2,
                    })
                    .layouter(layouter)
                    .show(ui)
            },
        )
        .inner;
    let response = output.response.response.clone();

    paint_gutter_chrome(ui, &output, gutter_width);
    eguicode::paint_gutter(ui, &output, code_theme, marks);
    track_response(
        "editor.source",
        &response,
        WidgetMeta {
            role: WidgetRoleMeta::TextEdit {
                multiline: true,
                password: false,
            },
            label: Some("source".to_string()),
            value: Some(WidgetValue::Text(source.clone())),
            layout: Some(capture_layout(ui, &response)),
            visible: ui.is_visible() && ui.is_rect_visible(response.rect),
            ..WidgetMeta::default()
        },
    );
    response
}

/// Paints the gutter's background fill and divider line. `eguicode`'s own
/// `paint_gutter` only draws the numbers; the sandbox keeps its own chrome
/// around them.
fn paint_gutter_chrome(ui: &Ui, output: &TextEditOutput, gutter_width: f32) {
    let editor_rect = output.response.response.rect;
    let gutter_rect = Rect::from_min_max(
        editor_rect.min,
        egui::pos2(editor_rect.min.x + gutter_width, editor_rect.max.y),
    );
    let painter = ui.painter();
    painter.rect_filled(gutter_rect, 0.0, theme::EDITOR_BG);
    painter.vline(
        gutter_rect.max.x - 1.0,
        gutter_rect.y_range(),
        Stroke::new(1.0, theme::DIVIDER.gamma_multiply(0.6)),
    );
}

fn show_samples_menu(ui: &mut Ui, state: &mut AppState) {
    ui.set_min_width(180.0);
    for (index, sample) in SAMPLES.iter().enumerate() {
        let response = ui
            .dev_button(
                format!("samples.item.{index}"),
                RichText::new(sample.name).color(theme::TEXT),
            )
            .on_hover_text(sample.description);
        if response.clicked() {
            state.load_sample(sample);
        }
    }
}

fn show_diagnostic(ui: &mut Ui, index: usize, diagnostic: &Diagnostic) {
    let (severity_name, color) = match diagnostic.severity {
        Severity::Error => ("error", theme::BAD),
        Severity::Warning => ("warning", theme::WARN),
    };

    eguidev::container(ui, format!("diagnostics.{index}"), |ui| {
        egui::Frame::new()
            .fill(theme::PANEL)
            .corner_radius(6)
            .inner_margin(Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.dev_label(
                        format!("diagnostics.{index}.severity"),
                        RichText::new(severity_name).color(color).strong().small(),
                    );
                    ui.dev_label(
                        format!("diagnostics.{index}.location"),
                        RichText::new(format!(
                            "line {}, column {}",
                            diagnostic.span.line, diagnostic.span.column
                        ))
                        .color(theme::MUTED)
                        .small(),
                    );
                });
                ui.dev_label(
                    format!("diagnostics.{index}.message"),
                    RichText::new(&diagnostic.message).color(theme::TEXT),
                );
                if let Some(help) = &diagnostic.help {
                    ui.dev_label(
                        format!("diagnostics.{index}.help"),
                        RichText::new(format!("help: {help}")).color(theme::MUTED),
                    );
                }
            });
        ui.add_space(6.0);
    });
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use std::{env, fs, path::Path, process};

    #[cfg(not(target_arch = "wasm32"))]
    use clap::Parser;

    use super::AppState;
    #[cfg(not(target_arch = "wasm32"))]
    use super::NativeArgs;

    // Syntax highlighting and diagnostic-mark tests live in `highlight.rs`,
    // beside the code they exercise.

    #[test]
    fn app_state_checks_initial_source() {
        let state = AppState::with_source("--!strict\nprint(missing)".to_string());

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
        let source = "print(\"loaded\")\n";
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
