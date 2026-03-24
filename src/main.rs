use clap::Parser;
use crossterm::event;
use ratatui::DefaultTerminal;
use std::sync::mpsc;
use std::time::Duration;
use void::input::{Command, handle_user_input};
use void::stream::{StreamEvent, stream_response};
use void::types::{AppState, Message};
use void::ui::{render, spinner_len};

#[derive(Parser)]
#[command(name = "void")]
struct Cli {
    #[arg(short, long, default_value = "7777")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut terminal = ratatui::init();
    let result = app(&mut terminal, cli.port).await;
    ratatui::restore();
    result
}

async fn app(terminal: &mut DefaultTerminal, port: u16) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut state = AppState {
        input: String::new(),
        messages: Vec::new(),
        port,
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
                Command::Exit => break,
                Command::InsertChar(ch) => state.input.push(ch),
                Command::ClearInput => state.input.clear(),
                Command::SubmitInput(msg) => {
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
                        let _ = stream_response(messages, tx, port).await;
                    });
                }
                Command::None => {}
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
            state.spinner_idx = (state.spinner_idx + 1) % spinner_len();
        }
    }

    Ok(())
}
