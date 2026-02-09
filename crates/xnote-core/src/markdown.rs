use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownBlockKind {
    Heading(u8),
    Paragraph,
    CodeFence,
    Quote,
    List,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownBlock {
    pub kind: MarkdownBlockKind,
    pub start_offset: usize,
    pub end_offset: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownSummary {
    pub headings: Vec<(u8, String)>,
    pub links: Vec<String>,
    pub code_fence_count: usize,
    pub block_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownParseResult {
    pub summary: MarkdownSummary,
    pub blocks: Vec<MarkdownBlock>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarkdownDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownDiagnostic {
    pub line: usize,
    pub severity: MarkdownDiagnosticSeverity,
    pub message: String,
}

pub trait MarkdownDiagnosticsProvider {
    fn provide(&self, text: &str) -> Vec<MarkdownDiagnostic>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownInvalidationWindow {
    pub start: usize,
    pub end: usize,
}

impl MarkdownInvalidationWindow {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start.min(end),
            end: start.max(end),
        }
    }

    pub fn from_edit(
        edit_range: Range<usize>,
        inserted_len: usize,
        new_document_len: usize,
        context_bytes: usize,
    ) -> Self {
        let start = edit_range.start.saturating_sub(context_bytes);
        let raw_end = edit_range
            .start
            .saturating_add(inserted_len)
            .saturating_add(context_bytes);
        let end = raw_end.min(new_document_len);
        Self::new(start.min(new_document_len), end)
    }

    pub fn merge(&mut self, other: &Self) {
        self.start = self.start.min(other.start);
        self.end = self.end.max(other.end);
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }
}

pub fn parse_markdown(text: &str) -> MarkdownParseResult {
    parse_markdown_internal(text, 0)
}

pub fn parse_markdown_summary(text: &str) -> MarkdownSummary {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(text, options);
    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut code_fence_count = 0usize;
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut block_count = 0usize;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    block_count += 1;
                    heading_stack.push((heading_level_to_u8(level), String::new()));
                }
                Tag::Paragraph | Tag::BlockQuote(_) | Tag::List(_) => {
                    block_count += 1;
                }
                Tag::CodeBlock(_) => {
                    block_count += 1;
                    code_fence_count += 1;
                }
                Tag::Link { dest_url, .. } => {
                    links.push(dest_url.to_string());
                }
                _ => {}
            },
            Event::Text(t) | Event::Code(t) => {
                if let Some((_level, text)) = heading_stack.last_mut() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_level, text)) = heading_stack.last_mut() {
                    text.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = heading_stack.pop() {
                    let heading_text = text.trim();
                    if !heading_text.is_empty() {
                        headings.push((level, heading_text.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    MarkdownSummary {
        headings,
        links,
        code_fence_count,
        block_count,
    }
}

pub fn lint_markdown(text: &str) -> Vec<MarkdownDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut heading_one_line: Option<usize> = None;
    let mut previous_heading_level: Option<u8> = None;
    let mut code_fence_open_line: Option<usize> = None;

    for (ix, line) in text.lines().enumerate() {
        let line_no = ix + 1;
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if code_fence_open_line.is_some() {
                code_fence_open_line = None;
            } else {
                code_fence_open_line = Some(line_no);
            }
        }

        if let Some(level) = markdown_heading_level(trimmed) {
            if level == 1 {
                if let Some(first_h1) = heading_one_line {
                    diagnostics.push(MarkdownDiagnostic {
                        line: line_no,
                        severity: MarkdownDiagnosticSeverity::Warning,
                        message: format!("multiple H1 headings (first at line {first_h1})"),
                    });
                } else {
                    heading_one_line = Some(line_no);
                }
            }

            if let Some(prev) = previous_heading_level {
                if level > prev + 1 {
                    diagnostics.push(MarkdownDiagnostic {
                        line: line_no,
                        severity: MarkdownDiagnosticSeverity::Warning,
                        message: format!("heading level jump from H{prev} to H{level}"),
                    });
                }
            }
            previous_heading_level = Some(level);
        }

        if line.chars().count() > 200 {
            diagnostics.push(MarkdownDiagnostic {
                line: line_no,
                severity: MarkdownDiagnosticSeverity::Info,
                message: "long line (> 200 chars)".to_string(),
            });
        }
    }

    if let Some(open_line) = code_fence_open_line {
        diagnostics.push(MarkdownDiagnostic {
            line: open_line,
            severity: MarkdownDiagnosticSeverity::Error,
            message: "unclosed code fence".to_string(),
        });
    }

    diagnostics.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then_with(|| b.severity.cmp(&a.severity))
            .then_with(|| a.message.cmp(&b.message))
    });
    diagnostics
}

pub fn lint_markdown_with_providers(
    text: &str,
    providers: &[&dyn MarkdownDiagnosticsProvider],
) -> Vec<MarkdownDiagnostic> {
    let mut out = lint_markdown(text);
    for provider in providers {
        out.extend(provider.provide(text));
    }
    out.sort_by(|a, b| {
        a.line
            .cmp(&b.line)
            .then_with(|| b.severity.cmp(&a.severity))
            .then_with(|| a.message.cmp(&b.message))
    });
    out
}

pub fn parse_markdown_window(
    text: &str,
    window: &MarkdownInvalidationWindow,
) -> MarkdownParseResult {
    let clamped_start = floor_char_boundary(text, window.start.min(text.len()));
    let clamped_end = ceil_char_boundary(text, window.end.min(text.len()));

    if clamped_start >= clamped_end {
        return MarkdownParseResult {
            summary: MarkdownSummary {
                headings: Vec::new(),
                links: Vec::new(),
                code_fence_count: 0,
                block_count: 0,
            },
            blocks: Vec::new(),
        };
    }

    let slice = &text[clamped_start..clamped_end];
    parse_markdown_internal(slice, clamped_start)
}

fn parse_markdown_internal(text: &str, base_offset: usize) -> MarkdownParseResult {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(text, options).into_offset_iter();

    let mut headings = Vec::new();
    let mut links = Vec::new();
    let mut code_fence_count = 0usize;
    let mut blocks = Vec::new();

    #[derive(Clone, Debug)]
    struct OpenBlock {
        kind: MarkdownBlockKind,
        start_offset: usize,
        text: String,
        heading_level: Option<u8>,
    }

    let mut open_stack: Vec<OpenBlock> = Vec::new();

    for (event, range) in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    open_stack.push(OpenBlock {
                        kind: MarkdownBlockKind::Heading(heading_level_to_u8(level)),
                        start_offset: base_offset + range.start,
                        text: String::new(),
                        heading_level: Some(heading_level_to_u8(level)),
                    });
                }
                Tag::Paragraph => {
                    open_stack.push(OpenBlock {
                        kind: MarkdownBlockKind::Paragraph,
                        start_offset: base_offset + range.start,
                        text: String::new(),
                        heading_level: None,
                    });
                }
                Tag::CodeBlock(_) => {
                    code_fence_count += 1;
                    open_stack.push(OpenBlock {
                        kind: MarkdownBlockKind::CodeFence,
                        start_offset: base_offset + range.start,
                        text: String::new(),
                        heading_level: None,
                    });
                }
                Tag::BlockQuote(_) => {
                    open_stack.push(OpenBlock {
                        kind: MarkdownBlockKind::Quote,
                        start_offset: base_offset + range.start,
                        text: String::new(),
                        heading_level: None,
                    });
                }
                Tag::List(_) => {
                    open_stack.push(OpenBlock {
                        kind: MarkdownBlockKind::List,
                        start_offset: base_offset + range.start,
                        text: String::new(),
                        heading_level: None,
                    });
                }
                Tag::Link { dest_url, .. } => {
                    links.push(dest_url.to_string());
                }
                _ => {}
            },
            Event::End(tag_end) => {
                let should_pop = matches!(
                    tag_end,
                    TagEnd::Heading(_)
                        | TagEnd::Paragraph
                        | TagEnd::CodeBlock
                        | TagEnd::BlockQuote(_)
                        | TagEnd::List(_)
                );
                if should_pop {
                    if let Some(open) = open_stack.pop() {
                        if let Some(level) = open.heading_level {
                            let heading_text = open.text.trim().to_string();
                            if !heading_text.is_empty() {
                                headings.push((level, heading_text));
                            }
                        }
                        blocks.push(MarkdownBlock {
                            kind: open.kind,
                            start_offset: open.start_offset,
                            end_offset: base_offset + range.end,
                            text: open.text.trim().to_string(),
                        });
                    }
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some(last) = open_stack.last_mut() {
                    if !last.text.is_empty() {
                        last.text.push(' ');
                    }
                    last.text.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(last) = open_stack.last_mut() {
                    last.text.push(' ');
                }
            }
            _ => {}
        }
    }

    let summary = MarkdownSummary {
        headings,
        links,
        code_fence_count,
        block_count: blocks.len(),
    };

    MarkdownParseResult { summary, blocks }
}

fn floor_char_boundary(text: &str, mut idx: usize) -> usize {
    idx = idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_char_boundary(text: &str, mut idx: usize) -> usize {
    idx = idx.min(text.len());
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn markdown_heading_level(line: &str) -> Option<u8> {
    let mut count = 0u8;
    for c in line.chars() {
        if c == '#' {
            count = count.saturating_add(1);
            if count > 6 {
                return None;
            }
        } else {
            break;
        }
    }
    if count == 0 {
        return None;
    }

    let marker = "#".repeat(count as usize);
    if line.starts_with(&format!("{marker} ")) {
        Some(count)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProvider;

    impl MarkdownDiagnosticsProvider for TestProvider {
        fn provide(&self, _text: &str) -> Vec<MarkdownDiagnostic> {
            vec![MarkdownDiagnostic {
                line: 2,
                severity: MarkdownDiagnosticSeverity::Info,
                message: "provider-message".to_string(),
            }]
        }
    }

    #[test]
    fn parse_markdown_extracts_headings_links_and_blocks() {
        let doc = "# Title\n\nHello [x](https://example.com)\n\n```rs\nfn main(){}\n```\n";
        let parsed = parse_markdown(doc);
        assert_eq!(parsed.summary.headings.len(), 1);
        assert_eq!(parsed.summary.headings[0], (1, "Title".to_string()));
        assert_eq!(parsed.summary.links.len(), 1);
        assert_eq!(parsed.summary.links[0], "https://example.com".to_string());
        assert_eq!(parsed.summary.code_fence_count, 1);
        assert!(parsed.summary.block_count >= 3);
    }

    #[test]
    fn invalidation_window_clamps_and_merges() {
        let mut a = MarkdownInvalidationWindow::from_edit(10..14, 2, 40, 6);
        assert_eq!(a.as_range(), 4..18);

        let b = MarkdownInvalidationWindow::new(16, 30);
        a.merge(&b);
        assert_eq!(a.as_range(), 4..30);
        assert_eq!(a.len(), 26);
    }

    #[test]
    fn parse_markdown_window_offsets_blocks() {
        let doc = "prefix\n# Heading\n\nbody\n";
        let start = doc.find('#').expect("heading start");
        let window = MarkdownInvalidationWindow::new(start, doc.len());
        let parsed = parse_markdown_window(doc, &window);

        let first = parsed.blocks.first().expect("first block");
        assert_eq!(first.start_offset, start);
        assert!(parsed
            .blocks
            .iter()
            .all(|block| block.start_offset >= window.start));
    }

    #[test]
    fn parse_markdown_summary_matches_full_summary_counts() {
        let doc = "# A\n\n## B\n[link](https://x.dev)\n\n```rs\nfn main() {}\n```\n";
        let summary_only = parse_markdown_summary(doc);
        let full = parse_markdown(doc).summary;

        assert_eq!(summary_only.headings.len(), full.headings.len());
        assert_eq!(summary_only.links.len(), full.links.len());
        assert_eq!(summary_only.code_fence_count, full.code_fence_count);
        assert_eq!(summary_only.block_count, full.block_count);
    }

    #[test]
    fn lint_markdown_detects_heading_jump_and_unclosed_fence() {
        let doc = "# H1\n### H3\n```\nbody\n";
        let diagnostics = lint_markdown(doc);

        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("heading level jump")));
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("unclosed code fence")));
    }

    #[test]
    fn lint_markdown_with_provider_merges_results() {
        let doc = "# H1\n# Another\n";
        let provider = TestProvider;
        let diagnostics = lint_markdown_with_providers(doc, &[&provider]);

        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("multiple H1 headings")));
        assert!(diagnostics.iter().any(|d| d.message == "provider-message"));
    }
}
