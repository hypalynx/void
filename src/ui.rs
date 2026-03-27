use crate::render::render_message;
use crate::types::{AppState, DisplayRole};
use ratatui::Frame;
use ratatui::prelude::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};

const HORIZONTAL_MARGIN: u16 = 2;
const VERTICAL_MARGIN: u16 = 1;
const SPACING: u16 = 1;
const HORIZONTAL_INPUT_MARGIN: u16 = 2;
const VERTICAL_INPUT_MARGIN: u16 = 1;
pub const SPINNER_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

pub fn spinner_len() -> usize {
    SPINNER_CHARS.chars().count()
}

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    // Calculate input height based on number of lines
    let input_lines = state.input.matches('\n').count() + 1;
    let input_height = (2 + input_lines).min(12) as u16; // cap at 10 inner lines

    let layout = Layout::default()
        .vertical_margin(VERTICAL_MARGIN)
        .horizontal_margin(HORIZONTAL_MARGIN)
        .spacing(SPACING)
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Fill(1),
            Constraint::Length(input_height),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let input_block = Block::new().white().on_black().padding(Padding::new(
        HORIZONTAL_INPUT_MARGIN,
        HORIZONTAL_INPUT_MARGIN,
        VERTICAL_INPUT_MARGIN,
        VERTICAL_INPUT_MARGIN,
    ));
    let inner = input_block.inner(layout[1]);

    // Compute geometry for scrolling
    let area_width = layout[0].width.max(1) as usize;
    state.msg_area_height = layout[0].height;

    let mut lines = Vec::new();

    // Render messages
    for msg in &state.messages {
        // Render thinking first if present
        if let Some(_thinking) = &msg.thinking {
            let thinking_lines = if msg.thinking_lines.is_empty() {
                // Streaming thinking — render live
                render_message(_thinking)
                    .into_iter()
                    .map(|spans| {
                        let styled: Vec<Span> = spans
                            .into_iter()
                            .map(|s| {
                                let mut style = s.style.add_modifier(Modifier::ITALIC);
                                if style.fg.is_none() {
                                    style = style.fg(Color::DarkGray);
                                }
                                Span::styled(s.content, style)
                            })
                            .collect();
                        Line::from(styled)
                    })
                    .collect::<Vec<_>>()
            } else {
                // Cached — use directly
                msg.thinking_lines.clone()
            };
            lines.extend(thinking_lines);
        }

        // Render main content
        let msg_lines = if msg.lines.is_empty() {
            // Streaming message — render live
            render_message(&msg.content)
                .into_iter()
                .map(|spans| {
                    let colored: Vec<Span> = spans
                        .into_iter()
                        .map(|s| {
                            if s.style.fg.is_none() {
                                Span::styled(s.content, color_for_role(msg.role))
                            } else {
                                s
                            }
                        })
                        .collect();
                    Line::from(colored)
                })
                .collect::<Vec<_>>()
        } else {
            // Cached — use directly
            msg.lines.clone()
        };

        lines.extend(msg_lines);

        // Render diff if present (for Write/Edit tool results)
        if let Some(diff) = &msg.diff {
            let diff_lines = crate::render::render_diff(diff);
            for spans in diff_lines {
                lines.push(Line::from(spans));
            }
        }

        // Render detail if toggled
        if state.show_tool_detail && msg.role == DisplayRole::ToolActivity {
            if let Some(detail) = &msg.detail {
                let detail_lines = if msg.detail_lines.is_empty() {
                    // Not yet cached
                    render_message(detail)
                        .into_iter()
                        .map(|spans| Line::from(spans))
                        .collect::<Vec<_>>()
                } else {
                    // Cached
                    msg.detail_lines.clone()
                };
                lines.extend(detail_lines);
            }
        }

        lines.push(Line::from("")); // Blank line between messages
    }

    // Render tool status
    for status in &state.tool_status {
        let spinner_char = SPINNER_CHARS
            .chars()
            .nth(state.spinner_idx % spinner_len())
            .unwrap_or(' ');
        let status_line = format!("  {} {}", spinner_char, status);
        lines.push(Line::from(Span::styled(status_line, Color::Yellow)));
    }

    // Compute total rendered lines after word wrapping
    state.total_rendered_lines = lines
        .iter()
        .map(|line| {
            let char_len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            (char_len.max(1) + area_width - 1) / area_width
        })
        .sum();

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.scroll_offset, 0));
    frame.render_widget(paragraph, layout[0]);
    frame.render_widget(input_block, layout[1]);

    // Render input as a paragraph to support multiline with proper newline handling
    let input_para = Paragraph::new(state.input.as_str()).wrap(Wrap { trim: false });
    frame.render_widget(input_para, inner);

    // Build status line with metrics on the right
    let status_left = if let Some(_) = state.last_exit_press {
        if state.waiting {
            "Press again to cancel".to_string()
        } else {
            "Press again to exit".to_string()
        }
    } else if state.waiting {
        let spinner_char = SPINNER_CHARS.chars().nth(state.spinner_idx).unwrap_or(' ');
        format!("{} waiting...", spinner_char)
    } else {
        String::new()
    };

    let metrics = format!("{:.1}ms {:.0}fps", state.last_draw_ms, state.fps);

    // Create a full-width status line with left and right content
    let status_area = layout[2];
    let full_width = status_area.width as usize;
    let metrics_width = metrics.len();
    let left_width = full_width.saturating_sub(metrics_width + 1);

    let mut status_line = status_left.clone();
    if status_line.len() < left_width {
        status_line.push_str(&" ".repeat(left_width - status_line.len()));
    }
    status_line.push_str(&metrics);

    let status_para = Paragraph::new(status_line).fg(Color::DarkGray);
    frame.render_widget(status_para, layout[2]);

    // Calculate 2D cursor position for multiline input
    let text_before = &state.input[..state.cursor];
    let cursor_row = text_before.matches('\n').count();
    let line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let cursor_col = state.cursor - line_start;
    frame.set_cursor_position((inner.x + cursor_col as u16, inner.y + cursor_row as u16));
}

fn color_for_role(role: DisplayRole) -> Color {
    match role {
        DisplayRole::User => Color::Cyan,
        DisplayRole::Assistant => Color::White,
        DisplayRole::ToolActivity => Color::Yellow,
    }
}
