use crate::render::render_message;
use crate::types::AppState;
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
    let layout = Layout::default()
        .vertical_margin(VERTICAL_MARGIN)
        .horizontal_margin(HORIZONTAL_MARGIN)
        .spacing(SPACING)
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Fill(1),
            Constraint::Length(3),
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
    for msg in &state.messages {
        match msg {
            crate::types::Message::User { content } => {
                let role_color = Color::Cyan;
                for spans in render_message(content) {
                    let colored: Vec<Span> = spans
                        .into_iter()
                        .map(|s| {
                            if s.style.fg.is_none() {
                                Span::styled(s.content, role_color)
                            } else {
                                s
                            }
                        })
                        .collect();
                    lines.push(Line::from(colored));
                }
            }
            crate::types::Message::Assistant {
                content,
                thinking,
                ..
            } => {
                let role_color = Color::White;
                if let Some(t) = thinking {
                    for spans in render_message(t) {
                        let italic_spans: Vec<Span> = spans
                            .into_iter()
                            .map(|s| {
                                let mut style = s.style.add_modifier(Modifier::ITALIC);
                                if style.fg.is_none() {
                                    style = style.fg(Color::DarkGray);
                                }
                                Span::styled(s.content, style)
                            })
                            .collect();
                        lines.push(Line::from(italic_spans));
                    }
                }
                for spans in render_message(content) {
                    let colored: Vec<Span> = spans
                        .into_iter()
                        .map(|s| {
                            if s.style.fg.is_none() {
                                Span::styled(s.content, role_color)
                            } else {
                                s
                            }
                        })
                        .collect();
                    lines.push(Line::from(colored));
                }
            }
            crate::types::Message::ToolResult {
                tool_call_id,
                content,
            } => {
                let role_color = Color::Green;
                lines.push(Line::from(vec![
                    Span::styled(format!("[Tool: {}", tool_call_id), role_color),
                    Span::raw("]"),
                ]));
                for spans in render_message(content) {
                    let colored: Vec<Span> = spans
                        .into_iter()
                        .map(|s| {
                            if s.style.fg.is_none() {
                                Span::styled(s.content, role_color)
                            } else {
                                s
                            }
                        })
                        .collect();
                    lines.push(Line::from(colored));
                }
            }
        }
        lines.push(Line::from("")); // Blank line between messages
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
    frame.render_widget(state.input.as_str(), inner);

    // Build status line with metrics on the right
    let status_left = if state.waiting {
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

    frame.render_widget(status_line, layout[2]);

    let cursor_x = inner.x + state.cursor as u16;
    let cursor_y = inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));
}
