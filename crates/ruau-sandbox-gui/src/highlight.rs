//! Editor syntax highlighting and diagnostic marks, built on `eguicode` and
//! `ruau-highlight`.

use eguicode::{
    CodeTheme, LineMark, MarkStyle, Marks, Severity as MarkSeverity, SpanMark, highlight::Class,
};
use ruau_sandbox::{Diagnostic, Severity};

use crate::theme;

/// The editor's monospace font, shared by source text and the gutter.
pub fn editor_font() -> egui::FontId {
    egui::FontId::new(13.0, egui::FontFamily::Monospace)
}

/// Classes that keep the sandbox's plain source color: the lexer-based
/// classifier distinguishes more classes than the scanner `eguicode`
/// replaces, but the sandbox palette only ever painted keywords, numbers,
/// strings, comments, and builtins apart from plain text.
const PLAIN_CLASSES: [Class; 10] = [
    Class::Name,
    Class::Function,
    Class::Field,
    Class::Type,
    Class::Literal,
    Class::Attribute,
    Class::Operator,
    Class::Punctuation,
    Class::Key,
    Class::Invalid,
];

/// Builds the editor's `CodeTheme` from the sandbox's own palette.
#[must_use]
pub fn code_theme() -> CodeTheme {
    let font = editor_font();
    let mut code_theme = CodeTheme::dark(font.clone());

    code_theme.set_class(Class::Keyword, theme::KEYWORD, font.clone());
    code_theme.set_class(Class::Number, theme::NUMBER, font.clone());
    code_theme.set_class(Class::String, theme::STRING, font.clone());
    code_theme.set_class(Class::Comment, theme::COMMENT, font.clone());
    code_theme.set_class(Class::Builtin, theme::MACRO, font.clone());
    for class in PLAIN_CLASSES {
        code_theme.set_class(class, theme::TEXT, font.clone());
    }

    code_theme.gutter_color = theme::GUTTER;
    code_theme.gutter_font = font;

    code_theme.set_mark_style(
        MarkSeverity::Error,
        MarkStyle {
            line_fill: theme::BAD.gamma_multiply(0.16),
            number_color: theme::BAD,
            underline: egui::Stroke::new(1.5, theme::BAD),
        },
    );
    code_theme.set_mark_style(
        MarkSeverity::Warning,
        MarkStyle {
            line_fill: theme::WARN.gamma_multiply(0.16),
            number_color: theme::WARN,
            underline: egui::Stroke::new(1.5, theme::WARN),
        },
    );
    code_theme
}

/// Converts sandbox diagnostics into `eguicode` marks: a background fill for
/// each diagnostic's line, plus an underline for its span.
#[must_use]
pub fn diagnostics_to_marks(diagnostics: &[Diagnostic]) -> Marks {
    let mut marks = Marks::default();
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            Severity::Error => MarkSeverity::Error,
            Severity::Warning => MarkSeverity::Warning,
        };
        let line = diagnostic.span.line.saturating_sub(1);
        marks.lines.push(LineMark { line, severity });
        marks.spans.push(SpanMark::one_based(
            diagnostic.span.line,
            diagnostic.span.column,
            diagnostic.span.len.max(1),
            severity,
        ));
    }
    marks
}

#[cfg(test)]
mod tests {
    use eguicode::highlight::{self, Language};
    use ruau_sandbox::check;

    use super::{code_theme, diagnostics_to_marks};
    use crate::theme;

    #[test]
    fn syntax_highlighting_classifies_luau_tokens() {
        let source = "local n = 42\nprint(\"value = \" .. n) -- output\n";
        let code_theme = code_theme();
        let marks = diagnostics_to_marks(&[]);
        let highlights = highlight::highlight(source, Language::Luau);
        let job = eguicode::layout_job(source, &highlights, &code_theme, &marks);
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
    fn comments_end_at_the_line_break() {
        let source = "-- note\nlocal x = 1";
        let code_theme = code_theme();
        let marks = diagnostics_to_marks(&[]);
        let highlights = highlight::highlight(source, Language::Luau);
        let job = eguicode::layout_job(source, &highlights, &code_theme, &marks);
        let colors = job
            .sections
            .iter()
            .map(|section| section.format.color)
            .collect::<Vec<_>>();

        assert!(colors.contains(&theme::COMMENT));
        assert!(
            colors.contains(&theme::KEYWORD),
            "keyword after a comment line should not be swallowed by the comment"
        );
    }

    #[test]
    fn diagnostics_mark_error_lines() {
        let source = "--!strict\nprint(missing)";
        let code_theme = code_theme();
        let marks = diagnostics_to_marks(&check(source).diagnostics);
        let highlights = highlight::highlight(source, Language::Luau);
        let job = eguicode::layout_job(source, &highlights, &code_theme, &marks);

        assert_eq!(job.text, source, "layout must reconstruct the source");
        assert!(
            job.sections
                .iter()
                .any(|section| section.format.background != egui::Color32::TRANSPARENT),
            "error line should have a background tint"
        );
        assert!(
            job.sections
                .iter()
                .any(|section| section.format.underline != egui::Stroke::NONE),
            "error span should be underlined"
        );
    }
}
