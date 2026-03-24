use clap::Parser;
use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use echo::llm::Message;
use echo::stream::{StreamEvent, stream_response};
use ratatui::prelude::{Constraint, Direction, Layout};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::sync::mpsc;
use std::time::Duration;

const HORIZONTAL_MARGIN: u16 = 2;
const VERTICAL_MARGIN: u16 = 1;
const SPACING: u16 = 1;
const HORIZONTAL_INPUT_MARGIN: u16 = 2;
const VERTICAL_INPUT_MARGIN: u16 = 1;
const SPINNER_CHARS: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";

#[derive(Parser)]
#[command(name = "echo")]
struct Cli {
    #[arg(short, long, default_value = "7777")]
    port: u16,
}

enum InputCommand {
    InsertChar(char),
    ClearInput,
    SubmitInput(String),
    Exit,
    None,
}

struct AppState {
    input: String,
    messages: Vec<Message>,
    rx: mpsc::Receiver<StreamEvent>,
    tx: mpsc::Sender<StreamEvent>,
    waiting: bool,
    spinner_idx: usize,
    current_stream_message_idx: Option<usize>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let result = app(&mut terminal).await;
    ratatui::restore();
    result
}

async fn app(terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut state = AppState {
        input: String::new(),
        messages: Vec::new(),
        rx,
        tx,
        waiting: false,
        spinner_idx: 0,
        current_stream_message_idx: None,
    };

    loop {
        terminal.draw(|frame| render(frame, &state))?;

        if event::poll(Duration::from_millis(50))?
            && let Some(key) = event::read()?.as_key_press_event()
        {
            match handle_user_input(key, &state.input) {
                InputCommand::Exit => break,
                InputCommand::InsertChar(ch) => insert(&mut state.input, ch),
                InputCommand::ClearInput => state.input.clear(),
                InputCommand::SubmitInput(msg) => {
                    state.messages.push(Message {
                        role: "user".to_string(),
                        content: msg,
                        thinking: None,
                    });
                    state.input.clear();
                    state.waiting = true;
                    state.spinner_idx = 0;

                    let messages = state.messages.clone();
                    let tx = state.tx.clone();
                    tokio::spawn(async move {
                        let _ = stream_response(messages, tx).await;
                    });
                }
                InputCommand::None => {}
            }
        }

        // Drain tokens from the channel
        while let Ok(event) = state.rx.try_recv() {
            match event {
                StreamEvent::Token(token) => {
                    if state.current_stream_message_idx.is_none() {
                        state.messages.push(Message {
                            role: "assistant".to_string(),
                            content: String::new(),
                            thinking: None,
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        state.messages[idx].content.push_str(&token);
                    }
                }
                StreamEvent::Thinking(thinking) => {
                    if state.current_stream_message_idx.is_none() {
                        state.messages.push(Message {
                            role: "assistant".to_string(),
                            content: String::new(),
                            thinking: Some(String::new()),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        if let Some(ref mut t) = state.messages[idx].thinking {
                            t.push_str(&thinking);
                        }
                    }
                }
                StreamEvent::Done => {
                    state.waiting = false;
                    state.current_stream_message_idx = None;
                }
            }
        }

        // Advance spinner
        if state.waiting {
            state.spinner_idx = (state.spinner_idx + 1) % SPINNER_CHARS.len();
        }
    }

    Ok(())
}

fn handle_user_input(key: KeyEvent, input: &str) -> InputCommand {
    if key.kind == KeyEventKind::Press {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                InputCommand::ClearInput
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                InputCommand::Exit
            }
            KeyCode::Char(to_insert) => InputCommand::InsertChar(to_insert),
            KeyCode::Enter => InputCommand::SubmitInput(input.to_string()),
            KeyCode::Esc => InputCommand::Exit,
            _ => InputCommand::None,
        }
    } else {
        InputCommand::None
    }
}

fn render(frame: &mut Frame, state: &AppState) {
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

    let cursor_x = inner.x + state.input.len() as u16;
    let cursor_y = inner.y;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn insert(input: &mut String, ch: char) {
    input.push(ch);
}
