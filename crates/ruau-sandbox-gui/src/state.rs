use eguidev::ScrollAreaState;
use ruau_sandbox::{CheckResult, DEFAULT_SOURCE, RunResult, Sample, check, execute};

/// Mutable app state shared with eguidev fixtures.
#[derive(Debug)]
pub struct AppState {
    /// Current editor buffer.
    pub source: String,
    /// Current checker result.
    pub check_result: CheckResult,
    /// Last run result.
    pub run_result: RunResult,
    pub(crate) run_history: Vec<RunRecord>,
    pub(crate) next_run_number: usize,
    pub(crate) editor_scroll: ScrollAreaState,
    pub(crate) output_scroll: ScrollAreaState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_source(DEFAULT_SOURCE.to_string())
    }
}

impl AppState {
    pub(crate) fn with_source(source: String) -> Self {
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

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn load_sample(&mut self, sample: &Sample) {
        self.source = sample.source.to_string();
        self.check_result = check(&self.source);
        self.clear_runs();
    }

    pub(crate) fn refresh_check(&mut self) {
        self.check_result = check(&self.source);
    }

    pub(crate) fn run(&mut self) {
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

    pub(crate) fn clear_runs(&mut self) {
        self.clear_output();
        self.editor_scroll.reset();
    }

    pub(crate) fn clear_output(&mut self) {
        self.run_result = RunResult::default();
        self.run_history.clear();
        self.next_run_number = 1;
        self.output_scroll.reset();
    }

    pub(crate) fn has_runs(&self) -> bool {
        !self.run_history.is_empty()
    }

    pub(crate) fn output_history_text(&self) -> String {
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
pub struct RunRecord {
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
