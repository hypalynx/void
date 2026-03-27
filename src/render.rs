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

pub fn render_message(text: &str, max_width: usize) -> Vec<Vec<Span<'static>>> {
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

    // Find all table blocks (consecutive lines with | characters)
    let table_blocks = find_table_blocks(text);

    // Merge code blocks and table blocks, sorted by start position
    let mut all_blocks: Vec<(usize, usize, BlockKind)> = Vec::new();
    for block in &code_blocks {
        all_blocks.push((block.start, block.end, BlockKind::Code));
    }
    for (start, end) in &table_blocks {
        all_blocks.push((*start, *end, BlockKind::Table));
    }
    all_blocks.sort_by_key(|(start, _, _)| *start);

    // Process text in segments (prose, code blocks, and table blocks)
    let mut pos = 0;

    for (block_start, block_end, kind) in all_blocks {
        if block_start < pos {
            // Overlapping block, skip
            continue;
        }

        // Process prose before the block
        if pos < block_start {
            let prose = &text[pos..block_start];
            for line_text in prose.lines() {
                let spans = parse_markdown_line(line_text);
                result.push(spans);
            }
        }

        match kind {
            BlockKind::Code => {
                // Find the code block details
                if let Some(block) = code_blocks.iter().find(|b| b.start == block_start) {
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
                }
            }
            BlockKind::Table => {
                let table_text = &text[block_start..block_end];
                let table_lines = render_table_block(table_text, max_width);
                for spans in table_lines {
                    result.push(spans);
                }
            }
        }

        // Move past the block
        pos = match kind {
            BlockKind::Code => {
                if let Some(block) = code_blocks.iter().find(|b| b.start == block_start) {
                    block.end + 3
                } else {
                    block_end
                }
            }
            BlockKind::Table => block_end,
        };
    }

    // Process remaining prose after last block
    if pos < text.len() {
        let prose = &text[pos..];
        for line_text in prose.lines() {
            let spans = parse_markdown_line(line_text);
            result.push(spans);
        }
    }

    result
}

#[derive(Clone, Copy)]
enum BlockKind {
    Code,
    Table,
}

fn find_table_blocks(text: &str) -> Vec<(usize, usize)> {
    let mut tables = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        // Look for potential table start (line with | that isn't a code fence)
        if lines[i].contains('|') && !lines[i].trim().starts_with("```") {
            let table_start_line = i;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().starts_with("```") {
                i += 1;
            }
            let table_end_line = i;
            // Require at least 2 rows for a table
            if table_end_line - table_start_line >= 2 {
                // Convert line indices to byte positions
                let mut byte_start = 0;
                let mut byte_end = 0;
                let mut current_line = 0;
                let mut pos = 0;

                for line in text.lines() {
                    if current_line == table_start_line {
                        byte_start = pos;
                    }
                    if current_line == table_end_line - 1 {
                        byte_end = pos + line.len();
                        break;
                    }
                    pos += line.len() + 1; // +1 for \n
                    current_line += 1;
                }

                if byte_start < byte_end {
                    tables.push((byte_start, byte_end));
                }
            }
        } else {
            i += 1;
        }
    }

    tables
}

fn render_table_block(table_text: &str, max_width: usize) -> Vec<Vec<Span<'static>>> {
    let lines: Vec<&str> = table_text.lines().collect();

    // Parse rows, splitting by | and trimming
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in &lines {
        let cells: Vec<String> = line
            .split('|')
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        if !cells.is_empty() {
            rows.push(cells);
        }
    }

    if rows.is_empty() {
        return Vec::new();
    }

    // Find max column count
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    // Skip separator rows (rows that are all dashes or contain only -, :, |)
    let data_rows: Vec<&Vec<String>> = rows
        .iter()
        .filter(|row| {
            !row.iter().all(|cell| {
                cell.chars().all(|c| c == '-' || c == ':' || c == '|' || c.is_whitespace())
            })
        })
        .collect();

    if data_rows.is_empty() {
        return Vec::new();
    }

    // Border overhead: 1 (left) + 2*col_count (padding) + (col_count-1) (internal │) + 1 (right)
    // = 1 + 2*col_count + col_count - 1 + 1 = 1 + 3*col_count
    let border_overhead = 1 + 3 * col_count;
    let available_width = max_width.saturating_sub(border_overhead);

    // Calculate visible width (excluding markdown formatting)
    let visible_width = |text: &str| {
        let mut width = 0;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '*' || c == '_' || c == '`' {
                continue;
            }
            if c == '[' {
                let mut link_text_len = 0;
                while let Some(c) = chars.next() {
                    if c == ']' { break; }
                    if c != '*' && c != '_' && c != '`' {
                        link_text_len += 1;
                    }
                }
                while let Some(c) = chars.next() {
                    if c == ')' { break; }
                }
                width += link_text_len;
            } else {
                width += 1;
            }
        }
        width
    };

    // Calculate initial max width per column from visible content
    let mut widths = vec![0; col_count];
    for row in &data_rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(visible_width(cell));
        }
    }

    // Ensure minimum width of 1 for all columns
    for i in 0..col_count {
        widths[i] = widths[i].max(1);
    }

    // Calculate total content width
    let total_content_width: usize = widths.iter().sum();

    // If too wide, scale down proportionally
    if total_content_width > available_width && available_width > col_count * 3 {
        let min_width = 3;
        let extra_space = available_width.saturating_sub(col_count * min_width);
        
        let total_original: usize = widths.iter().sum();
        if total_original > 0 {
            for i in 0..col_count {
                let proportion = widths[i] as f64 / total_original as f64;
                let scaled = (extra_space as f64 * proportion) as usize + min_width;
                widths[i] = scaled.max(min_width).min(available_width);
            }
        }
    } else if total_content_width > available_width {
        let w = available_width / col_count;
        for i in 0..col_count {
            widths[i] = w.max(1);
        }
    }

    // Build result as flattened lines
    let mut result: Vec<Vec<Span<'static>>> = Vec::new();
    
    // Top border
    let top_border = {
        let parts: Vec<String> = (0..col_count)
            .map(|i| "─".repeat(widths[i] + 2))
            .collect();
        let border = format!("╭{}╮", parts.join("┬"));
        vec![Span::styled(border, Style::default().fg(Color::DarkGray))]
    };
    result.push(top_border);

    // Data rows
    for row in &data_rows {
        let cell_spans: Vec<Vec<Vec<Span<'static>>>> = (0..col_count)
            .map(|i| {
                let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                let spans = parse_markdown_line(cell);
                wrap_spans(spans, widths[i])
            })
            .collect();

        let max_lines = cell_spans.iter().map(|c| c.len()).max().unwrap_or(1);
        
        for line_idx in 0..max_lines {
            let mut line_spans: Vec<Span<'static>> = Vec::new();
            
            line_spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            
            for i in 0..col_count {
                let cell_content = cell_spans.get(i)
                    .and_then(|lines| lines.get(line_idx))
                    .cloned()
                    .unwrap_or_else(|| vec![Span::raw(" ")]);
                
                let visible_len: usize = cell_content.iter().map(|s| s.content.chars().count()).sum();
                let padding = widths[i].saturating_sub(visible_len);
                
                line_spans.push(Span::raw(" "));
                line_spans.extend(cell_content);
                if padding > 0 {
                    line_spans.push(Span::raw(" ".repeat(padding)));
                }
                line_spans.push(Span::raw(" "));
                line_spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
            }
            
            result.push(line_spans);
        }
    }

    // Bottom border
    let bottom_border = {
        let parts: Vec<String> = (0..col_count)
            .map(|i| "─".repeat(widths[i] + 2))
            .collect();
        let border = format!("╰{}╯", parts.join("┴"));
        vec![Span::styled(border, Style::default().fg(Color::DarkGray))]
    };
    result.push(bottom_border);

    result
}

fn wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Vec<Span<'static>>> {
    let effective_width = max_width.max(1);
    
    if spans.is_empty() {
        return vec![vec![Span::raw(" ")]];
    }
    
    let mut lines: Vec<Vec<Span<'static>>> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0;
    
    let mut words: Vec<(String, Style)> = Vec::new();
    for span in spans {
        let text = &span.content;
        let style = span.style;
        for word in text.split_whitespace() {
            words.push((word.to_string(), style));
        }
    }
    
    for (word, style) in words {
        let word_len = word.chars().count();
        
        if current_line.is_empty() {
            if word_len > effective_width {
                for chunk in word.chars().collect::<Vec<_>>().chunks(effective_width) {
                    let chunk_str: String = chunk.iter().collect();
                    lines.push(vec![Span::styled(chunk_str, style)]);
                }
            } else {
                current_line.push(Span::styled(word, style));
                current_width = word_len;
            }
        } else if current_width + 1 + word_len <= effective_width {
            current_line.push(Span::styled(" ".to_string(), style));
            current_line.push(Span::styled(word, style));
            current_width += 1 + word_len;
        } else {
            lines.push(current_line);
            current_line = Vec::new();
            
            if word_len > effective_width {
                for chunk in word.chars().collect::<Vec<_>>().chunks(effective_width) {
                    let chunk_str: String = chunk.iter().collect();
                    if current_line.is_empty() {
                        current_line.push(Span::styled(chunk_str, style));
                    } else {
                        lines.push(current_line);
                        current_line = vec![Span::styled(chunk_str, style)];
                    }
                }
                current_width = current_line.iter().map(|s| s.content.chars().count()).sum();
            } else {
                current_line.push(Span::styled(word, style));
                current_width = word_len;
            }
        }
    }
    
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    
    if lines.is_empty() {
        lines.push(vec![Span::raw(" ")]);
    }
    
    lines
}

/// Render a file diff with line numbers, +/- markers, and syntax highlighting
pub fn render_diff(diff: &crate::types::FileDiff) -> Vec<Vec<Span<'static>>> {
    use crate::types::DiffLineKind;

    let mut result = Vec::new();
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

    // Detect syntax from file extension, fallback to plaintext
    let syntax = syntax_set
        .find_syntax_for_file(&diff.path)
        .ok()
        .flatten()
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);

    for (hunk_idx, hunk) in diff.hunks.iter().enumerate() {
        // Add hunk separator if not the first hunk
        if hunk_idx > 0 {
            result.push(vec![Span::styled(
                "~~~ (hunk separator) ~~~".to_string(),
                Style::default().fg(Color::DarkGray),
            )]);
        }

        for line in &hunk.lines {
            let lineno_str = format!("{:>4}", line.lineno);
            let lineno_span = Span::styled(lineno_str, Style::default().fg(Color::DarkGray));

            // Marker span: + (green), - (red), or space (default)
            let marker_span = match line.kind {
                DiffLineKind::Added => {
                    Span::styled("+".to_string(), Style::default().fg(Color::Green))
                }
                DiffLineKind::Removed => Span::styled("-".to_string(), Style::default().fg(Color::Red)),
                DiffLineKind::Context => Span::raw(" "),
            };

            // Syntax highlight the content
            let ranges = highlighter
                .highlight_line(&line.content, syntax_set)
                .unwrap_or_default();

            let mut content_spans: Vec<Span> = ranges
                .iter()
                .map(|(style, text)| {
                    let fg_color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

                    // For added/removed lines, override the color with green/red
                    let final_color = match line.kind {
                        DiffLineKind::Added => Color::Green,
                        DiffLineKind::Removed => Color::Red,
                        DiffLineKind::Context => fg_color,
                    };

                    Span::styled(text.to_string(), Style::default().fg(final_color))
                })
                .collect();

            // If no spans (empty line), add a placeholder
            if content_spans.is_empty() {
                content_spans.push(Span::raw(""));
            }

            // Build final line: [lineno] [marker] [content...]
            let mut line_spans = vec![lineno_span, Span::raw(" "), marker_span, Span::raw(" ")];
            line_spans.extend(content_spans);

            result.push(line_spans);
        }
    }

    result
}