use crate::types::AppState;
use ratatui::Frame;
use ratatui::prelude::{Constraint, Direction, Layout};
use ratatui::style::{Color, Stylize};
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

pub fn render(frame: &mut Frame, state: &AppState) {
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

    let mut lines = Vec::new();
    for msg in &state.messages {
        let style = match msg.role.as_str() {
            "user" => Color::Cyan,
            "assistant" => Color::White,
            _ => Color::White,
        };
        if let Some(thinking) = &msg.thinking {
            for line in thinking.lines() {
                lines.push(Line::from(Span::styled(line, Color::DarkGray)));
            }
        }
        for line in msg.content.lines() {
            lines.push(Line::from(Span::styled(line, style)));
        }
        lines.push(Line::from("")); // Blank line between messages
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, layout[0]);
    frame.render_widget(input_block, layout[1]);
    frame.render_widget(state.input.as_str(), inner);

    let status = if state.waiting {
        let spinner_char = SPINNER_CHARS.chars().nth(state.spinner_idx).unwrap_or(' ');
        format!("{} waiting...", spinner_char)
    } else {
        String::new()
    };

    frame.render_widget(status, layout[2]);

    let cursor_x = inner.x + state.cursor as u16;
    let cursor_y = inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));
}
