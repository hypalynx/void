use clap::Parser;
use crossterm::event;
use ratatui::DefaultTerminal;
use std::sync::mpsc;
use std::time::Duration;
use void::input::{
    Command, handle_user_input, delete_backward_char, delete_forward_char, move_backward_char,
    move_forward_char, move_backward_word, move_forward_word, move_start_of_line,
    move_end_of_line, kill_backward_word, kill_backward_line, yank,
};
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
        cursor: 0,
        clipboard: String::new(),
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
                Command::InsertChar(ch) => {
                    state.input.insert(state.cursor, ch);
                    state.cursor += 1;
                }
                Command::ClearInput => {
                    state.input.clear();
                    state.cursor = 0;
                }
                Command::DeleteBackwardChar => {
                    (state.input, state.cursor) = delete_backward_char(&state.input, state.cursor);
                }
                Command::DeleteForwardChar => {
                    (state.input, state.cursor) = delete_forward_char(&state.input, state.cursor);
                }
                Command::MoveBackwardChar => {
                    state.cursor = move_backward_char(state.cursor);
                }
                Command::MoveForwardChar => {
                    state.cursor = move_forward_char(&state.input, state.cursor);
                }
                Command::MoveBackwardWord => {
                    state.cursor = move_backward_word(&state.input, state.cursor);
                }
                Command::MoveForwardWord => {
                    state.cursor = move_forward_word(&state.input, state.cursor);
                }
                Command::MoveStartOfLine => {
                    state.cursor = move_start_of_line();
                }
                Command::MoveEndOfLine => {
                    state.cursor = move_end_of_line(&state.input);
                }
                Command::KillBackwardWord => {
                    (state.input, state.cursor) = kill_backward_word(&state.input, state.cursor);
                }
                Command::KillBackwardLine => {
                    (state.input, state.cursor, state.clipboard) =
                        kill_backward_line(&state.input, state.cursor);
                }
                Command::Yank => {
                    (state.input, state.cursor) =
                        yank(&state.input, state.cursor, &state.clipboard.clone());
                }
                Command::SubmitInput(msg) => {
                    state.messages.push(Message {
                        role: "user".to_string(),
                        content: msg,
                        thinking: None,
                    });
                    state.input.clear();
                    state.cursor = 0;
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
