use std::collections::BTreeMap;

use egui::{Color32, FontFamily, FontId, Stroke, TextFormat, text::LayoutJob};
use ruau_sandbox::{Diagnostic, Severity};

use crate::theme;

/// Diagnostic decorations for a single source line.
#[derive(Debug, Default, Clone)]
pub struct LineMark {
    /// Whether the line has at least one error (as opposed to only warnings).
    pub(crate) has_error: bool,
    /// Byte ranges to underline: `(start, len, severity)`, zero-based within
    /// the line.
    spans: Vec<(usize, usize, Severity)>,
}

impl LineMark {
    fn background(&self) -> Color32 {
        let base = if self.has_error {
            theme::BAD
        } else {
            theme::WARN
        };
        base.gamma_multiply(0.16)
    }
}

pub fn collect_line_marks(diagnostics: &[Diagnostic]) -> BTreeMap<usize, LineMark> {
    let mut marks: BTreeMap<usize, LineMark> = BTreeMap::new();
    for diagnostic in diagnostics {
        let mark = marks.entry(diagnostic.span.line).or_default();
        if diagnostic.severity == Severity::Error {
            mark.has_error = true;
        }
        mark.spans.push((
            diagnostic.span.column.saturating_sub(1),
            diagnostic.span.len.max(1),
            diagnostic.severity,
        ));
    }
    for mark in marks.values_mut() {
        mark.spans.sort_by_key(|&(start, _, _)| start);
    }
    marks
}

pub fn highlight_source(source: &str, marks: &BTreeMap<usize, LineMark>) -> LayoutJob {
    let monospace = FontId::new(13.0, FontFamily::Monospace);
    let mut job = LayoutJob::default();

    for (index, line) in source.split_inclusive('\n').enumerate() {
        let (content, has_newline) = match line.strip_suffix('\n') {
            Some(content) => (content, true),
            None => (line, false),
        };
        append_line(&mut job, content, marks.get(&(index + 1)), &monospace);
        if has_newline {
            push(&mut job, "\n", &monospace, theme::TEXT);
        }
    }

    job.wrap.break_anywhere = false;
    job
}

fn append_line(job: &mut LayoutJob, line: &str, mark: Option<&LineMark>, font_id: &FontId) {
    let background = mark.map_or(Color32::TRANSPARENT, LineMark::background);
    let mut rest = line;
    let mut offset = 0;

    while !rest.is_empty() {
        let Some((token, kind, tail)) = next_token(rest) else {
            push_marked(job, rest, offset, theme::TEXT, background, mark, font_id);
            break;
        };
        push_marked(
            job,
            token,
            offset,
            token_color(kind),
            background,
            mark,
            font_id,
        );
        offset += token.len();
        rest = tail;
    }
}

/// Append a token, splitting it into underlined and plain segments where
/// diagnostic spans overlap it. `token_start` is the token's byte offset
/// within its line; mark spans use the same coordinates.
fn push_marked(
    job: &mut LayoutJob,
    token: &str,
    token_start: usize,
    color: Color32,
    background: Color32,
    mark: Option<&LineMark>,
    font_id: &FontId,
) {
    let mut cursor = 0;

    if let Some(mark) = mark {
        for &(span_start, span_len, severity) in &mark.spans {
            let rel_start = span_start.saturating_sub(token_start).max(cursor);
            let rel_end = (span_start + span_len).saturating_sub(token_start);
            let rel_start = ceil_char_boundary(token, rel_start);
            let rel_end = ceil_char_boundary(token, rel_end);
            if rel_end <= rel_start {
                continue;
            }
            if rel_start > cursor {
                push_format(
                    job,
                    &token[cursor..rel_start],
                    font_id,
                    color,
                    background,
                    Stroke::NONE,
                );
            }
            let underline_color = match severity {
                Severity::Error => theme::BAD,
                Severity::Warning => theme::WARN,
            };
            push_format(
                job,
                &token[rel_start..rel_end],
                font_id,
                color,
                background,
                Stroke::new(1.5, underline_color),
            );
            cursor = rel_end;
        }
    }

    if cursor < token.len() {
        push_format(
            job,
            &token[cursor..],
            font_id,
            color,
            background,
            Stroke::NONE,
        );
    }
}

/// Snap a byte index within `text` to the nearest valid char boundary at or
/// after it, clamped to the text length.
fn ceil_char_boundary(text: &str, mut index: usize) -> usize {
    if index > text.len() {
        return text.len();
    }
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
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
    if input.starts_with("--") {
        return Some((input, TokenKind::Comment, ""));
    }
    if first.is_ascii_digit() {
        let len = take_while(input, |ch| ch.is_ascii_digit());
        return Some((&input[..len], TokenKind::Number, &input[len..]));
    }
    if first == '_' || first.is_ascii_alphabetic() {
        let len = take_while(input, |ch| ch == '_' || ch.is_ascii_alphanumeric());
        let kind = if is_keyword(&input[..len]) {
            TokenKind::Keyword
        } else {
            TokenKind::Plain
        };
        let kind = if matches!(&input[..len], "print" | "warn" | "error") {
            TokenKind::Macro
        } else {
            kind
        };
        let token_len = len;
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
        "and"
            | "break"
            | "continue"
            | "do"
            | "else"
            | "elseif"
            | "end"
            | "false"
            | "for"
            | "function"
            | "if"
            | "in"
            | "local"
            | "nil"
            | "not"
            | "or"
            | "repeat"
            | "return"
            | "self"
            | "then"
            | "true"
            | "type"
            | "until"
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
    push_format(
        job,
        text,
        font_id,
        color,
        Color32::TRANSPARENT,
        Stroke::NONE,
    );
}

fn push_format(
    job: &mut LayoutJob,
    text: &str,
    font_id: &FontId,
    color: Color32,
    background: Color32,
    underline: Stroke,
) {
    if text.is_empty() {
        return;
    }
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: font_id.clone(),
            color,
            background,
            underline,
            ..Default::default()
        },
    );
}
