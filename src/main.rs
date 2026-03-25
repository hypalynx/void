use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind, MouseEventKind};
use crossterm::execute;
use ratatui::DefaultTerminal;
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};
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
    execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
    let result = app(&mut terminal, cli.port).await;
    execute!(io::stdout(), crossterm::event::DisableMouseCapture)?;
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
        last_render_ms: 0.0,
        fps: 0.0,
        scroll_offset: 0,
        total_rendered_lines: 0,
        msg_area_height: 0,
    };

    let mut frame_count = 0;
    let mut last_fps_time = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(16)); // ~60 FPS

    loop {
        interval.tick().await;

        let render_start = Instant::now();
        terminal.draw(|frame| render(frame, &mut state))?;
        state.last_render_ms = render_start.elapsed().as_secs_f64() * 1000.0;

        // Non-blocking input check
        if event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
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
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        state.scroll_offset = state.scroll_offset.saturating_sub(3);
                    }
                    MouseEventKind::ScrollDown => {
                        let max = state.total_rendered_lines
                            .saturating_sub(state.msg_area_height as usize) as u16;
                        state.scroll_offset = (state.scroll_offset + 3).min(max);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    let max = state.total_rendered_lines
                        .saturating_sub(state.msg_area_height as usize) as u16;
                    state.scroll_offset = state.scroll_offset.min(max);
                }
                _ => {}
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

        // Advance spinner every other frame
        if state.waiting {
            if frame_count % 2 == 0 {
                state.spinner_idx = (state.spinner_idx + 1) % spinner_len();
            }
        }

        frame_count += 1;
        let now = Instant::now();
        if now.duration_since(last_fps_time).as_secs() >= 1 {
            state.fps = frame_count as f64 / now.duration_since(last_fps_time).as_secs_f64();
            frame_count = 0;
            last_fps_time = now;
        }
    }

    Ok(())
}
