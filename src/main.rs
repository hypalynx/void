use clap::Parser;
use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use std::sync::mpsc;
use std::time::Duration;
use void::stream::{StreamEvent, stream_response};
use void::types::{AppState, Message};
use void::ui::{SPINNER_CHARS, render};

#[derive(Parser)]
#[command(name = "void")]
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
                InputCommand::InsertChar(ch) => state.input.push(ch),
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
