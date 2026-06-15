//! Shared checker and executor for the Ruau sandbox UI.
//!
//! The first engine is intentionally small and deterministic: it accepts a
//! Rust-shaped subset that can run in native and wasm builds without a compiler
//! subprocess. It gives the GUI real diagnostics, output, and source locations
//! while leaving room to swap in a fuller backend later.

#![allow(clippy::missing_docs_in_private_items)]

use std::collections::BTreeMap;

/// A default program shown in the editor on first launch.
pub const DEFAULT_SOURCE: &str = r#"fn main() {
    let width: i64 = 6;
    let height = 7;
    let area = width * height;

    println!("area = {}", area);
}"#;

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
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
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
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

/// Check source without returning execution output.
#[must_use]
pub fn check(source: &str) -> CheckResult {
    let analysis = analyze(source, Mode::Check);
    CheckResult {
        diagnostics: analysis.diagnostics,
    }
}

/// Execute source and capture output.
#[must_use]
pub fn execute(source: &str) -> RunResult {
    let analysis = analyze(source, Mode::Execute);
    RunResult {
        diagnostics: analysis.diagnostics,
        output: analysis.output,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Check,
    Execute,
}

#[derive(Debug, Default)]
struct Analysis {
    diagnostics: Vec<Diagnostic>,
    output: String,
}

#[derive(Debug, Clone)]
struct SourceLine {
    number: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct StatementLine {
    number: usize,
    text: String,
}

fn analyze(source: &str, mode: Mode) -> Analysis {
    let mut analysis = Analysis::default();
    let Some(lines) = extract_body(source, &mut analysis.diagnostics) else {
        return analysis;
    };

    let mut env = BTreeMap::new();
    let mut saw_statement = false;

    for line in &lines {
        let Some(statement) = statement_from_line(line) else {
            continue;
        };
        saw_statement = true;
        if let Some((name, value)) = parse_let(&statement, &env, &mut analysis.diagnostics) {
            env.insert(name, value);
        } else if let Some(output) = parse_print(&statement, &env, &mut analysis.diagnostics) {
            if mode == Mode::Execute {
                analysis.output.push_str(&output);
            }
        } else if !starts_like_supported_statement(&statement.text) {
            analysis.diagnostics.push(
                Diagnostic::error(
                    SourceSpan::new(statement.number, leading_column(&statement.text), 1),
                    "expected a `let` binding or `println!` call",
                )
                .with_help("supported statements are `let name = expr;` and `println!(...);`"),
            );
        }
    }

    if !saw_statement && analysis.diagnostics.is_empty() {
        analysis.diagnostics.push(
            Diagnostic::error(
                SourceSpan::new(1, 1, 1),
                "program does not contain any statements",
            )
            .with_help("add a `let` binding or a `println!` call inside `fn main`"),
        );
    }

    analysis
}

fn extract_body(source: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<Vec<SourceLine>> {
    if let Some(fn_start) = source.find("fn main") {
        let Some(open_rel) = source[fn_start..].find('{') else {
            diagnostics.push(
                Diagnostic::error(
                    line_col(source, fn_start),
                    "`fn main` is missing an opening `{`",
                )
                .with_help("write `fn main() { ... }`"),
            );
            return None;
        };
        let open = fn_start + open_rel;
        let Some(close) = matching_brace(source, open) else {
            diagnostics.push(Diagnostic::error(
                line_col(source, open),
                "`fn main` block is missing a closing `}`",
            ));
            return None;
        };
        return Some(source_lines(
            &source[open + 1..close],
            line_number(source, open + 1),
        ));
    }

    Some(source_lines(source, 1))
}

fn matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = source[open..].char_indices();

    for (rel, ch) in &mut chars {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + rel);
                }
            }
            _ => {}
        }
    }

    None
}

fn source_lines(source: &str, start_line: usize) -> Vec<SourceLine> {
    source
        .lines()
        .enumerate()
        .map(|(index, text)| SourceLine {
            number: start_line + index,
            text: text.to_string(),
        })
        .collect()
}

fn statement_from_line(line: &SourceLine) -> Option<StatementLine> {
    let trimmed = line.text.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }

    Some(StatementLine {
        number: line.number,
        text: strip_line_comment(&line.text).trim_end().to_string(),
    })
}

fn strip_line_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    let mut iter = line.char_indices().peekable();

    while let Some((idx, ch)) = iter.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
        } else if ch == '/' && iter.peek().is_some_and(|(_, next)| *next == '/') {
            return &line[..idx];
        }
    }

    line
}

fn starts_like_supported_statement(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("let ") || trimmed.starts_with("println!") || trimmed.starts_with("print!")
}

fn parse_let(
    statement: &StatementLine,
    env: &BTreeMap<String, i64>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, i64)> {
    let trimmed = statement.text.trim_start();
    if !trimmed.starts_with("let ") {
        return None;
    }

    let semicolonless = require_semicolon(statement, diagnostics)?;
    let rest = semicolonless
        .trim_start()
        .strip_prefix("let ")?
        .trim_start();
    let Some((left, expr)) = rest.split_once('=') else {
        diagnostics.push(Diagnostic::error(
            SourceSpan::new(statement.number, leading_column(&statement.text), 3),
            "`let` binding is missing `=`",
        ));
        return Some((String::new(), 0));
    };

    let (name, type_name) = parse_binding_name(left.trim(), statement, diagnostics)?;
    if let Some(type_name) = type_name
        && !matches!(type_name, "i64" | "i32" | "isize" | "u64" | "u32" | "usize")
    {
        diagnostics.push(
            Diagnostic::error(
                SourceSpan::new(
                    statement.number,
                    column_of(&statement.text, type_name),
                    type_name.len(),
                ),
                format!("unsupported integer type `{type_name}`"),
            )
            .with_help("use one of `i64`, `i32`, `isize`, `u64`, `u32`, or `usize`"),
        );
        return Some((String::new(), 0));
    }

    let base_col = column_of(&statement.text, expr);
    let value = ExprParser::new(expr, statement.number, base_col, env)
        .parse()
        .map_err(|diagnostic| diagnostics.push(diagnostic))
        .ok()?;

    Some((name.to_string(), value))
}

fn parse_binding_name<'a>(
    left: &'a str,
    statement: &StatementLine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(&'a str, Option<&'a str>)> {
    let (name, type_name) = match left.split_once(':') {
        Some((name, type_name)) => (name.trim(), Some(type_name.trim())),
        None => (left.trim(), None),
    };

    if is_ident(name) {
        return Some((name, type_name));
    }

    diagnostics.push(Diagnostic::error(
        SourceSpan::new(
            statement.number,
            column_of(&statement.text, left),
            left.len().max(1),
        ),
        format!("invalid binding name `{name}`"),
    ));
    None
}

fn parse_print(
    statement: &StatementLine,
    env: &BTreeMap<String, i64>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let trimmed = statement.text.trim_start();
    let (macro_name, newline) = if trimmed.starts_with("println!") {
        ("println!", true)
    } else if trimmed.starts_with("print!") {
        ("print!", false)
    } else {
        return None;
    };

    let semicolonless = require_semicolon(statement, diagnostics)?;
    let rest = semicolonless.trim_start().strip_prefix(macro_name)?.trim();
    if !rest.starts_with('(') || !rest.ends_with(')') {
        diagnostics.push(
            Diagnostic::error(
                SourceSpan::new(
                    statement.number,
                    column_of(&statement.text, macro_name),
                    macro_name.len(),
                ),
                format!("`{macro_name}` expects parentheses"),
            )
            .with_help(format!(
                "write `{macro_name}(...) ;` without the extra space before `;`"
            )),
        );
        return Some(String::new());
    }

    let inner = &rest[1..rest.len() - 1];
    let args = split_args(inner);
    if args.is_empty() {
        diagnostics.push(Diagnostic::error(
            SourceSpan::new(statement.number, column_of(&statement.text, inner), 1),
            format!("`{macro_name}` needs at least one argument"),
        ));
        return Some(String::new());
    }

    let mut output = if is_string_literal(args[0]) {
        render_format_string(args[0], &args[1..], statement, env, diagnostics)
    } else {
        eval_print_expr(args[0], statement, env, diagnostics)
    }?;

    if newline {
        output.push('\n');
    }
    Some(output)
}

fn render_format_string(
    template_arg: &str,
    value_args: &[&str],
    statement: &StatementLine,
    env: &BTreeMap<String, i64>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let template = unescape_string(template_arg)
        .map_err(|message| {
            diagnostics.push(Diagnostic::error(
                SourceSpan::new(
                    statement.number,
                    column_of(&statement.text, template_arg),
                    template_arg.len(),
                ),
                message,
            ));
        })
        .ok()?;

    let placeholders = template.matches("{}").count();
    if placeholders != value_args.len() {
        diagnostics.push(
            Diagnostic::error(
                SourceSpan::new(
                    statement.number,
                    column_of(&statement.text, template_arg),
                    template_arg.len(),
                ),
                format!(
                    "format string expects {placeholders} values, but {} were provided",
                    value_args.len()
                ),
            )
            .with_help("this sandbox supports `{}` placeholders"),
        );
        return None;
    }

    let mut values = Vec::with_capacity(value_args.len());
    for arg in value_args {
        values.push(eval_print_expr(arg, statement, env, diagnostics)?);
    }

    let mut rendered = template;
    for value in values {
        rendered = rendered.replacen("{}", &value, 1);
    }

    Some(rendered)
}

fn eval_print_expr(
    expr: &str,
    statement: &StatementLine,
    env: &BTreeMap<String, i64>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let base_col = column_of(&statement.text, expr);
    ExprParser::new(expr, statement.number, base_col, env)
        .parse()
        .map(|value| value.to_string())
        .map_err(|diagnostic| diagnostics.push(diagnostic))
        .ok()
}

fn require_semicolon<'a>(
    statement: &'a StatementLine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a str> {
    let trimmed = statement.text.trim_end();
    if let Some(stripped) = trimmed.strip_suffix(';') {
        return Some(stripped);
    }

    diagnostics.push(
        Diagnostic::error(
            SourceSpan::new(statement.number, statement.text.len().saturating_add(1), 1),
            "statement is missing a trailing `;`",
        )
        .with_help("Rust statements in this sandbox must end with `;`"),
    );
    None
}

fn is_string_literal(value: &str) -> bool {
    value.trim_start().starts_with('"')
}

fn unescape_string(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('"') || !trimmed.ends_with('"') || trimmed.len() < 2 {
        return Err("expected a string literal".to_string());
    }

    let mut out = String::new();
    let mut chars = trimmed[1..trimmed.len() - 1].chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => out.push('\n'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => return Err(format!("unsupported escape sequence `\\{other}`")),
            None => return Err("unterminated escape sequence".to_string()),
        }
    }

    Ok(out)
}

fn split_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let arg = input[start..idx].trim();
                if !arg.is_empty() {
                    args.push(arg);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    let arg = input[start..].trim();
    if !arg.is_empty() {
        args.push(arg);
    }
    args
}

struct ExprParser<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    base_col: usize,
    env: &'a BTreeMap<String, i64>,
}

impl<'a> ExprParser<'a> {
    fn new(input: &'a str, line: usize, base_col: usize, env: &'a BTreeMap<String, i64>) -> Self {
        Self {
            input,
            pos: 0,
            line,
            base_col,
            env,
        }
    }

    fn parse(mut self) -> Result<i64, Diagnostic> {
        let value = self.expr()?;
        self.skip_ws();
        if !self.is_done() {
            return Err(self.error("unexpected token in expression", 1));
        }
        Ok(value)
    }

    fn expr(&mut self) -> Result<i64, Diagnostic> {
        let mut value = self.term()?;
        loop {
            self.skip_ws();
            if self.take('+') {
                value = value
                    .checked_add(self.term()?)
                    .ok_or_else(|| self.error("integer addition overflowed", 1))?;
            } else if self.take('-') {
                value = value
                    .checked_sub(self.term()?)
                    .ok_or_else(|| self.error("integer subtraction overflowed", 1))?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn term(&mut self) -> Result<i64, Diagnostic> {
        let mut value = self.factor()?;
        loop {
            self.skip_ws();
            if self.take('*') {
                value = value
                    .checked_mul(self.factor()?)
                    .ok_or_else(|| self.error("integer multiplication overflowed", 1))?;
            } else if self.take('/') {
                let rhs = self.factor()?;
                if rhs == 0 {
                    return Err(self.error("division by zero", 1));
                }
                value /= rhs;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn factor(&mut self) -> Result<i64, Diagnostic> {
        self.skip_ws();
        if self.take('-') {
            return self
                .factor()?
                .checked_neg()
                .ok_or_else(|| self.error("integer negation overflowed", 1));
        }

        if self.take('(') {
            let value = self.expr()?;
            self.skip_ws();
            if !self.take(')') {
                return Err(self.error("missing closing `)`", 1));
            }
            return Ok(value);
        }

        match self.peek() {
            Some(ch) if ch.is_ascii_digit() => self.number(),
            Some(ch) if is_ident_start(ch) => self.identifier(),
            Some(_) => Err(self.error(
                "expected an integer, identifier, or parenthesized expression",
                1,
            )),
            None => Err(self.error("expected an expression", 1)),
        }
    }

    fn number(&mut self) -> Result<i64, Diagnostic> {
        let start = self.pos;
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }
        self.input[start..self.pos]
            .parse()
            .map_err(|_| self.error("integer literal is too large", self.pos - start))
    }

    fn identifier(&mut self) -> Result<i64, Diagnostic> {
        let start = self.pos;
        self.bump();
        while self.peek().is_some_and(is_ident_continue) {
            self.bump();
        }
        let name = &self.input[start..self.pos];
        self.env.get(name).copied().ok_or_else(|| {
            Diagnostic::error(
                SourceSpan::new(self.line, self.base_col + start, name.len()),
                format!("cannot find value `{name}` in this scope"),
            )
            .with_help("create it first with `let name = ...;`")
        })
    }

    fn error(&self, message: impl Into<String>, len: usize) -> Diagnostic {
        Diagnostic::error(
            SourceSpan::new(self.line, self.base_col + self.pos, len),
            message,
        )
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    fn take(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn bump(&mut self) {
        let Some(ch) = self.peek() else {
            return;
        };
        self.pos += ch.len_utf8();
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn is_done(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn is_ident(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(is_ident_start) && chars.all(is_ident_continue)
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn leading_column(text: &str) -> usize {
    text.len() - text.trim_start().len() + 1
}

fn column_of(haystack: &str, needle: &str) -> usize {
    haystack.find(needle).map_or(1, |idx| idx + 1)
}

fn line_number(source: &str, byte: usize) -> usize {
    source[..byte.min(source.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn line_col(source: &str, byte: usize) -> SourceSpan {
    let byte = byte.min(source.len());
    let line_start = source[..byte].rfind('\n').map_or(0, |idx| idx + 1);
    SourceSpan::new(line_number(source, byte), byte - line_start + 1, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_default_program() {
        let result = execute(DEFAULT_SOURCE);
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output, "area = 42\n");
    }

    #[test]
    fn reports_unknown_identifier() {
        let result = check("fn main() {\n    println!(missing);\n}");
        assert!(!result.is_ok());
        assert!(result.diagnostics[0].message.contains("missing"));
        assert_eq!(result.diagnostics[0].span.line, 2);
    }

    #[test]
    fn reports_division_by_zero() {
        let result = check("let x = 1 / 0;");
        assert!(!result.is_ok());
        assert_eq!(result.diagnostics[0].message, "division by zero");
    }

    #[test]
    fn formats_placeholders() {
        let result = execute("let a = 2;\nlet b = 3;\nprintln!(\"{} + {}\", a, b);");
        assert!(result.is_ok(), "{:?}", result.diagnostics);
        assert_eq!(result.output, "2 + 3\n");
    }
}
