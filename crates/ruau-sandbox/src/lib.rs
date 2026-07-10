//! Shared checker and executor for the Ruau sandbox UI.
//!
//! This crate is intentionally a thin adapter over the real Ruau parser,
//! checker, compiler, and VM. The editor buffer is treated as one Luau chunk:
//! declarations do not run unless called, top-level statements run in order,
//! and typed Luau syntax is handled by Ruau rather than by a local parser.

#![allow(clippy::missing_docs_in_private_items)]

use std::sync::{Arc, Mutex};

use ruau::{
    bytecode::CompileError,
    source::{ModuleId, Source},
    surface::{PrepareGraphError, PreparedGraphRunError, PreparedGraphScript, Surface, VmConfig},
    typecheck::{
        DiagnosticRecord, GraphDiagnostics, OneBasedDiagnosticLocation, Severity as RuauSeverity,
    },
    vm::{
        CallOptions, ExecError, MarshaledPair, MarshaledValue, PrintSink, SinkQuota, TableLayout,
        classify_marshaled_table,
    },
};

const CHUNK_NAME: &str = "sandbox.luau";
const DEFAULT_GAS: u64 = 50_000_000;
const DEFAULT_MAX_MEMORY_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_PRINT_MAX_BYTES: usize = 64 * 1024;
const DEFAULT_PRINT_MAX_CALLS: usize = 1024;

/// A default program shown in the editor on first launch.
pub const DEFAULT_SOURCE: &str = r#"--!strict

function area(width: number, height: number): number
    return width * height
end

print("area = " .. area(6, 7))"#;

/// A named example program offered by the sandbox UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    /// Short name shown in the samples menu.
    pub name: &'static str,
    /// One-line description of what the sample demonstrates.
    pub description: &'static str,
    /// Program source loaded into the editor.
    pub source: &'static str,
}

/// Example programs offered by the sandbox UI. The first entry is the default
/// program.
pub const SAMPLES: &[Sample] = &[
    Sample {
        name: "Area",
        description: "Multiply two bindings and print the result.",
        source: DEFAULT_SOURCE,
    },
    Sample {
        name: "Functions",
        description: "Declare global and local helpers, then call them from the chunk.",
        source: r#"--!strict

function greet(name: string): string
    return "hello, " .. name
end

local function shout(message: string): string
    return string.upper(message)
end

print(shout(greet("Ada")))"#,
    },
    Sample {
        name: "Math helpers",
        description: "Compose two typed function declarations.",
        source: r#"--!strict

function square(value: number): number
    return value * value
end

function sum_of_squares(left: number, right: number): number
    return square(left) + square(right)
end

print("sum = " .. sum_of_squares(3, 4))"#,
    },
    Sample {
        name: "Tables",
        description: "Iterate a table and print formatted output.",
        source: r#"--!strict

local scores = {
    ada = 11,
    grace = 17,
    linus = 13,
}

for name, score in pairs(scores) do
    print(name .. ": " .. score)
end"#,
    },
    Sample {
        name: "Types",
        description: "Use a typed record and a typed function.",
        source: r#"--!strict

type Point = {
    x: number,
    y: number,
}

local function magnitude(point: Point): number
    return math.sqrt(point.x * point.x + point.y * point.y)
end

print("magnitude = " .. magnitude({ x = 3, y = 4 }))"#,
    },
    Sample {
        name: "Broken program",
        description: "Deliberate mistakes that show real Ruau diagnostics.",
        source: r#"--!strict

local width: number = 6
print(height)
print(width + "wide")"#,
    },
];

/// Find a sample by name.
#[must_use]
pub fn sample_by_name(name: &str) -> Option<&'static Sample> {
    SAMPLES.iter().find(|sample| sample.name == name)
}

/// Severity of a source diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The program cannot be checked or executed successfully.
    Error,
    /// The program is valid, but a likely issue was found.
    Warning,
}

/// A line and column range in the source editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// One-based line number.
    pub line: usize,
    /// One-based column number.
    pub column: usize,
    /// Length in bytes for the highlighted range.
    pub len: usize,
}

impl SourceSpan {
    /// Construct a new source span.
    #[must_use]
    pub const fn new(line: usize, column: usize, len: usize) -> Self {
        Self { line, column, len }
    }
}

/// A user-facing diagnostic produced by checking or execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Diagnostic severity.
    pub severity: Severity,
    /// Primary message.
    pub message: String,
    /// Source range to highlight.
    pub span: SourceSpan,
    /// Optional follow-up detail or correction.
    pub help: Option<String>,
}

impl Diagnostic {
    /// Construct an error diagnostic.
    #[must_use]
    pub fn error(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            help: None,
        }
    }

    /// Construct a warning diagnostic.
    #[must_use]
    pub fn warning(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            help: None,
        }
    }

    /// Add a help note to this diagnostic.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// Result of checking a source buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckResult {
    /// Diagnostics found while checking.
    pub diagnostics: Vec<Diagnostic>,
}

impl CheckResult {
    /// Whether no errors were found.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        diagnostics_are_ok(&self.diagnostics)
    }
}

/// Result of executing a source buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunResult {
    /// Diagnostics found before or during execution.
    pub diagnostics: Vec<Diagnostic>,
    /// Captured standard output.
    pub output: String,
}

impl RunResult {
    /// Whether the program ran without errors.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        diagnostics_are_ok(&self.diagnostics)
    }
}

/// Check source without returning execution output.
#[must_use]
pub fn check(source: &str) -> CheckResult {
    match prepare(source) {
        Ok(prepared) => CheckResult {
            diagnostics: diagnostics_from_ruau(prepared.diagnostics()),
        },
        Err(error) => CheckResult {
            diagnostics: diagnostics_from_prepare_error(&error),
        },
    }
}

/// Execute source as one Luau chunk and capture `print` output.
#[must_use]
pub fn execute(source: &str) -> RunResult {
    let prepared = match prepare(source) {
        Ok(prepared) => prepared,
        Err(error) => {
            return RunResult {
                diagnostics: diagnostics_from_prepare_error(&error),
                output: String::new(),
            };
        }
    };

    let surface = Surface::new();
    let mut vm = match surface
        .vm_builder(&VmConfig::metered_untrusted(
            0,
            DEFAULT_GAS,
            DEFAULT_MAX_MEMORY_BYTES,
        ))
        .build()
    {
        Ok(vm) => vm,
        Err(error) => {
            return RunResult {
                diagnostics: vec![Diagnostic::error(
                    SourceSpan::new(1, 1, 1),
                    format!("failed to build Ruau VM: {error}"),
                )],
                output: String::new(),
            };
        }
    };

    let prints = Arc::new(Mutex::new(Vec::<String>::new()));
    let options = CallOptions::new().print_sink_with_quota(
        print_sink(Arc::clone(&prints)),
        SinkQuota {
            max_bytes: Some(DEFAULT_PRINT_MAX_BYTES),
            max_calls: Some(DEFAULT_PRINT_MAX_CALLS),
        },
    );

    let diagnostics = match prepared.run_in_with_options(&mut vm, options) {
        Ok(_) => Vec::new(),
        Err(error) => vec![diagnostic_from_run_error(&error)],
    };

    RunResult {
        diagnostics,
        output: prints_to_output(prints),
    }
}

fn prepare(source: &str) -> Result<PreparedGraphScript, PrepareGraphError> {
    Surface::new().prepare_graph(Source::text(ModuleId::new(CHUNK_NAME), source))
}

fn diagnostics_are_ok(diagnostics: &[Diagnostic]) -> bool {
    !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

fn diagnostics_from_prepare_error(error: &PrepareGraphError) -> Vec<Diagnostic> {
    let mut diagnostics = error
        .diagnostics()
        .map_or_else(Vec::new, diagnostics_from_ruau);
    if let Some(compile_error) = error.compile_error() {
        diagnostics.push(diagnostic_from_compile_error(compile_error));
    }
    if diagnostics.is_empty() {
        diagnostics.push(Diagnostic::error(
            SourceSpan::new(1, 1, 1),
            error.to_string(),
        ));
    }
    diagnostics
}

fn diagnostics_from_ruau(diagnostics: &GraphDiagnostics) -> Vec<Diagnostic> {
    diagnostics
        .records()
        .map(|record| diagnostic_from_record(record.diagnostic))
        .collect()
}

fn diagnostic_from_record(record: DiagnosticRecord) -> Diagnostic {
    let span = source_span_from_location(record.primary_location);
    let mut diagnostic = match record.severity {
        RuauSeverity::Error => Diagnostic::error(span, record.message),
        RuauSeverity::Warning | RuauSeverity::Info => Diagnostic::warning(span, record.message),
    };
    diagnostic.help = Some(record.category_label);
    diagnostic
}

fn diagnostic_from_compile_error(error: &CompileError) -> Diagnostic {
    let span = error
        .location()
        .map_or(SourceSpan::new(1, 1, 1), |location| {
            SourceSpan::new(
                location.begin.line as usize + 1,
                location.begin.column as usize + 1,
                diagnostic_len(
                    location.begin.line as usize,
                    location.begin.column as usize,
                    location.end.line as usize,
                    location.end.column as usize,
                ),
            )
        });
    Diagnostic::error(span, error.message().to_owned()).with_help(format!("{:?}", error.kind()))
}

fn diagnostic_from_run_error(error: &PreparedGraphRunError) -> Diagnostic {
    match error {
        PreparedGraphRunError::Load(error) => {
            Diagnostic::error(SourceSpan::new(1, 1, 1), format!("load failed: {error}"))
        }
        PreparedGraphRunError::Exec(error) => diagnostic_from_exec_error(error),
    }
}

fn diagnostic_from_exec_error(error: &ExecError) -> Diagnostic {
    let span = error
        .script_error()
        .and_then(|error| error.primary_frame())
        .and_then(|frame| frame.line)
        .map_or(SourceSpan::new(1, 1, 1), |line| {
            SourceSpan::new(line as usize, 1, 1)
        });
    Diagnostic::error(span, runtime_message(error))
}

fn runtime_message(error: &ExecError) -> String {
    let Some(error) = error.script_error() else {
        return error.message();
    };
    match error.value() {
        MarshaledValue::String(bytes) => String::from_utf8_lossy(bytes).into_owned(),
        MarshaledValue::Nil => "nil".to_owned(),
        MarshaledValue::Boolean(value) => value.to_string(),
        MarshaledValue::Number(value) => value.to_string(),
        MarshaledValue::Integer(value) => value.to_string(),
        MarshaledValue::Table(pairs) => marshaled_table_string_field(pairs, "message")
            .unwrap_or_else(|| "script raised table".to_owned()),
        value => format!("script raised {}", value.type_name()),
    }
}

fn marshaled_table_string_field(pairs: &[MarshaledPair], field: &str) -> Option<String> {
    if !matches!(
        classify_marshaled_table(pairs),
        TableLayout::StringMap { .. } | TableLayout::Mixed { .. }
    ) {
        return None;
    }
    pairs.iter().find_map(|pair| {
        let MarshaledValue::String(key) = &pair.key else {
            return None;
        };
        if key.as_slice() != field.as_bytes() {
            return None;
        }
        let MarshaledValue::String(value) = &pair.value else {
            return None;
        };
        Some(String::from_utf8_lossy(value).into_owned())
    })
}

fn source_span_from_location(location: OneBasedDiagnosticLocation) -> SourceSpan {
    if location.is_missing() {
        return SourceSpan::new(1, 1, 1);
    }
    SourceSpan::new(
        location.begin.line as usize,
        location.begin.column as usize,
        diagnostic_len(
            location.begin.line as usize,
            location.begin.column as usize,
            location.end.line as usize,
            location.end.column as usize,
        ),
    )
}

fn diagnostic_len(
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
) -> usize {
    if end_line == start_line {
        end_column.saturating_sub(start_column).max(1)
    } else {
        1
    }
}

fn print_sink(prints: Arc<Mutex<Vec<String>>>) -> PrintSink {
    Box::new(move |line| {
        let message = String::from_utf8_lossy(line)
            .trim_end_matches('\n')
            .to_owned();
        if let Ok(mut prints) = prints.lock() {
            prints.push(message);
        }
    })
}

fn prints_to_output(prints: Arc<Mutex<Vec<String>>>) -> String {
    let prints = match Arc::try_unwrap(prints) {
        Ok(prints) => prints.into_inner().unwrap_or_default(),
        Err(prints) => prints
            .lock()
            .map(|prints| prints.clone())
            .unwrap_or_default(),
    };
    if prints.is_empty() {
        String::new()
    } else {
        format!("{}\n", prints.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use ruau::source::{ModuleSourceError, ModuleSourceResult, SourceMetadata, SyncModuleSource};

    use super::*;

    #[derive(Default)]
    struct MutableSource {
        source: Mutex<Vec<u8>>,
        epoch: AtomicU64,
    }

    impl MutableSource {
        fn new(source: impl AsRef<[u8]>) -> Self {
            Self {
                source: Mutex::new(source.as_ref().to_vec()),
                epoch: AtomicU64::new(1),
            }
        }

        fn replace(&self, source: impl AsRef<[u8]>) {
            *self.source.lock().expect("source mutex") = source.as_ref().to_vec();
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
    }

    impl SyncModuleSource for MutableSource {
        fn resolve_sync(
            &self,
            _requester: Option<&ModuleId>,
            request: &[u8],
        ) -> ModuleSourceResult<ModuleId> {
            if request == b"dep" {
                return Ok(ModuleId::new("dep"));
            }
            Err(ModuleSourceError::MissingModule {
                id: ModuleId::from(request),
            })
        }

        fn read_sync(&self, id: &ModuleId) -> ModuleSourceResult<Vec<u8>> {
            if id.as_str() == Some("dep") {
                return Ok(self.source.lock().expect("source mutex").clone());
            }
            Err(ModuleSourceError::MissingModule { id: id.clone() })
        }

        fn metadata(&self, id: &ModuleId) -> SourceMetadata {
            SourceMetadata::new(format!("{}.luau", id.to_lossy_string()))
        }

        fn epoch(&self) -> u64 {
            self.epoch.load(Ordering::Relaxed)
        }
    }

    #[test]
    fn executes_default_program() {
        let result = execute(DEFAULT_SOURCE);
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output, "area = 42\n");
    }

    #[test]
    fn empty_source_is_valid() {
        let result = execute("\n-- no statements yet\n");
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert!(result.output.is_empty());
    }

    #[test]
    fn function_declaration_does_not_auto_run() {
        let result = execute("local function main()\n    print(\"ok\")\nend");
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert!(result.output.is_empty());
    }

    #[test]
    fn global_function_declaration_does_not_auto_run() {
        let result = execute("function main()\n    print(\"ok\")\nend");
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert!(result.output.is_empty());
    }

    #[test]
    fn executes_global_function_call_in_chunk() {
        let result = execute(
            r#"--!strict

function greet(name: string): string
    return "hello, " .. name
end

print(greet("Ada"))"#,
        );
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output, "hello, Ada\n");
    }

    #[test]
    fn executes_local_function_call_in_chunk() {
        let result = execute(
            r#"--!strict

local function square(value: number): number
    return value * value
end

print("square = " .. square(7))"#,
        );
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output, "square = 49\n");
    }

    #[test]
    fn accepts_typed_luau_syntax() {
        let result = execute(
            r#"--!strict

type Point = { x: number, y: number }

local function magnitude(point: Point): number
    return math.sqrt(point.x * point.x + point.y * point.y)
end

print("magnitude = " .. magnitude({ x = 3, y = 4 }))"#,
        );
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output, "magnitude = 5\n");
    }

    #[test]
    fn reports_typecheck_diagnostics() {
        let result = check("--!strict\nprint(height)");
        assert!(!result.is_ok());
        assert!(result.diagnostics[0].message.contains("height"));
        assert_eq!(result.diagnostics[0].span.line, 2);
    }

    #[test]
    fn execute_does_not_emit_output_when_checking_fails() {
        let result = execute("--!strict\nprint(height)");
        assert!(!result.is_ok());
        assert!(result.output.is_empty());
        assert!(result.diagnostics[0].message.contains("height"));
    }

    #[test]
    fn reports_compile_diagnostics() {
        let result = check("local function broken(");
        assert!(!result.is_ok());
        assert!(!result.diagnostics[0].message.is_empty());
    }

    #[test]
    fn reports_runtime_errors() {
        let result = execute("error(\"boom\")");
        assert!(!result.is_ok());
        assert!(result.output.is_empty());
        assert!(result.diagnostics[0].message.contains("boom"));
    }

    #[test]
    fn reports_structured_runtime_error_messages() {
        let result = execute("error({ message = \"structured boom\" })");
        assert!(!result.is_ok());
        assert_eq!(result.diagnostics[0].message, "structured boom");
    }

    #[test]
    fn reports_missing_dependency_diagnostics() {
        let result = check("return require('missing')");
        assert!(!result.is_ok());
        assert!(
            result.diagnostics[0].message.contains("missing"),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn prepared_graph_rejects_a_stale_source_epoch() {
        let modules = Arc::new(MutableSource::new("return 41"));
        let surface = Surface::builder()
            .module_source(modules.clone())
            .build()
            .expect("surface validates");
        let prepared = surface
            .prepare_graph(Source::text(
                ModuleId::new(CHUNK_NAME),
                "return require('dep') + 1",
            ))
            .expect("graph prepares");
        modules.replace("return 42");
        let mut vm = surface
            .vm_builder(&VmConfig::metered_untrusted(
                0,
                DEFAULT_GAS,
                DEFAULT_MAX_MEMORY_BYTES,
            ))
            .build()
            .expect("VM builds");

        let error = prepared.run_in(&mut vm).expect_err("stale graph fails");
        let diagnostic = diagnostic_from_run_error(&error);

        assert!(diagnostic.message.contains("epoch"), "{diagnostic:?}");
    }

    #[test]
    fn execution_is_bounded_by_gas() {
        let result = execute("while true do end");
        assert!(!result.is_ok());
        assert!(
            result.diagnostics[0].message.contains("budget"),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn print_calls_are_bounded_by_quota() {
        let result = execute("for _ = 1, 1025 do print('x') end");
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output.lines().count(), DEFAULT_PRINT_MAX_CALLS + 1);
        assert_eq!(result.output.lines().last(), Some("[output truncated]"));
    }

    #[test]
    fn samples_check_as_described() {
        for sample in SAMPLES {
            let result = check(sample.source);
            if sample.name == "Broken program" {
                assert!(!result.is_ok(), "broken sample should have errors");
            } else {
                assert!(
                    result.is_ok(),
                    "sample `{}` should check cleanly: {:?}",
                    sample.name,
                    result.diagnostics
                );
            }
        }
    }

    #[test]
    fn sample_lookup_finds_default() {
        let sample = sample_by_name("Area").expect("default sample");
        assert_eq!(sample.source, DEFAULT_SOURCE);
        assert!(sample_by_name("nope").is_none());
    }
}
