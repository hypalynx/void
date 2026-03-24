use pulldown_cmark::{Event as MdEvent, Parser as MdParser, Tag};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use std::sync::OnceLock;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

fn get_syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_nonewlines)
}

fn get_theme_set() -> &'static ThemeSet {
    static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub fn parse_markdown_line(text: &str) -> Vec<Span<'static>> {
    let parser = MdParser::new(text);
    let mut spans = Vec::new();
    let mut bold = false;
    let mut italic = false;

    for event in parser {
        match event {
            MdEvent::Start(tag) => match tag {
                Tag::Strong => bold = true,
                Tag::Emphasis => italic = true,
                _ => {}
            },
            MdEvent::End(tag) => match tag {
                Tag::Strong => bold = false,
                Tag::Emphasis => italic = false,
                _ => {}
            },
            MdEvent::Text(text) => {
                let s = text.to_string();
                let mut style = Style::default();
                if bold {
                    style = style.bold();
                    style = style.fg(Color::Yellow);
                } else if italic {
                    style = style.italic();
                }
                spans.push(Span::styled(s, style));
            }
            MdEvent::Code(text) => {
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                // Line breaks shouldn't happen in a single line
            }
            _ => {}
        }
    }

    spans
}

pub fn highlight_code_block(code: &str, language: &str) -> Vec<Vec<Span<'static>>> {
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    let theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .unwrap_or_else(|| {
            theme_set
                .themes
                .values()
                .next()
                .expect("at least one theme")
        });

    let syntax = syntax_set
        .find_syntax_by_token(language)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighted_lines = Vec::new();
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);

    for line in code.lines() {
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();

        let mut spans = Vec::new();
        for (style, text) in ranges {
            let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            spans.push(Span::styled(text.to_string(), Style::default().fg(color)));
        }
        if spans.is_empty() {
            spans.push(Span::raw(""));
        }
        highlighted_lines.push(spans);
    }

    highlighted_lines
}

pub fn render_message(text: &str) -> Vec<Vec<Span<'static>>> {
    let mut result = Vec::new();

    // Find all code block ranges
    #[derive(Clone, Copy)]
    struct CodeBlock {
        start: usize,
        end: usize,
        language: usize,
        language_len: usize,
    }

    let mut code_blocks = Vec::new();
    let mut in_fence = false;
    let mut fence_start = 0;
    let mut fence_lang_start = 0;
    let mut fence_lang_len = 0;

    let bytes = text.as_bytes();
    let mut i = 0;

    // Scan for complete code block fences
    while i < bytes.len() {
        if (i == 0 || bytes[i - 1] == b'\n') && i + 3 <= bytes.len() && &bytes[i..i + 3] == b"```" {
            if in_fence {
                code_blocks.push(CodeBlock {
                    start: fence_start,
                    end: i,
                    language: fence_lang_start,
                    language_len: fence_lang_len,
                });
                in_fence = false;
                i += 3;
            } else {
                in_fence = true;
                fence_start = i;
                i += 3;

                let line_end = bytes[i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|p| i + p)
                    .unwrap_or(bytes.len());
                fence_lang_start = i;
                fence_lang_len = line_end - i;
                i = line_end;
            }
        } else {
            i += 1;
        }
    }

    // Process text in segments (prose and code blocks)
    let mut pos = 0;

    for block in code_blocks {
        // Process prose before the block
        if pos < block.start {
            let prose = &text[pos..block.start];
            for line_text in prose.lines() {
                let spans = parse_markdown_line(line_text);
                result.push(spans);
            }
        }

        // Extract language tag
        let lang_bytes = &text.as_bytes()[block.language..block.language + block.language_len];
        let language = std::str::from_utf8(lang_bytes).unwrap_or("").trim();

        // Extract code block content (skip opening ``` line and closing ``` line)
        let fence_open_end = text[block.start..]
            .find('\n')
            .map(|p| block.start + p + 1)
            .unwrap_or(block.start + 3);
        let code_content = &text[fence_open_end..block.end];

        // Highlight code block lines
        let highlighted_lines = highlight_code_block(code_content, language);
        for spans in highlighted_lines {
            result.push(spans);
        }

        // Move past the closing fence
        pos = block.end + 3;
    }

    // Process remaining prose after last code block
    if pos < text.len() {
        let prose = &text[pos..];
        for line_text in prose.lines() {
            let spans = parse_markdown_line(line_text);
            result.push(spans);
        }
    }

    result
}
