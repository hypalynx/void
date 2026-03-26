use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind, MouseEventKind};
use crossterm::execute;
use ratatui::DefaultTerminal;
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use void::input::{
    Command, delete_backward_char, delete_forward_char, handle_user_input, kill_backward_line,
    kill_backward_word, move_backward_char, move_backward_word, move_end_of_line,
    move_forward_char, move_forward_word, move_start_of_line, yank,
};
use void::stream::{StreamEvent, stream_response};
use void::tool;
use void::types::{AppState, Message, ToolCall, ToolResultInfo};
use void::ui::{draw, spinner_len};

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

async fn execute_tool_calls(tool_calls: Vec<ToolCall>) -> Vec<ToolResultInfo> {
    let mut tasks = Vec::new();

    for tool_call in tool_calls {
        let task = tokio::spawn(async move {
            let args = match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                &tool_call.function.arguments,
            ) {
                Ok(args) => args,
                Err(_) => serde_json::Map::new(),
            };

            let tool_obj = tool::ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                args,
            };

            let result = match tool::execute(&tool_obj) {
                Ok(output) => output,
                Err(e) => format!("Tool execution error: {}", e),
            };

            ToolResultInfo {
                tool_call_id: tool_call.id,
                content: result,
            }
        });

        tasks.push(task);
    }

    let mut results = Vec::new();
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }

    results
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
        last_draw_ms: 0.0,
        fps: 0.0,
        scroll_offset: 0,
        target_scroll_offset: 0,
        total_rendered_lines: 0,
        msg_area_height: 0,
    };

    let mut frame_count = 0;
    let mut last_fps_time = Instant::now();
    let mut interval = tokio::time::interval(Duration::from_millis(16)); // ~60 FPS

    loop {
        interval.tick().await;

        // Smooth scroll animation: interpolate toward target
        let diff = (state.target_scroll_offset as i32) - (state.scroll_offset as i32);
        if diff != 0 {
            let delta = (diff as f64 * 0.25).round() as i32;
            state.scroll_offset = ((state.scroll_offset as i32) + delta).max(0) as u16;
        }

        let draw_start = Instant::now();
        terminal.draw(|frame| draw(frame, &mut state))?;
        state.last_draw_ms = draw_start.elapsed().as_secs_f64() * 1000.0;

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
                            (state.input, state.cursor) =
                                delete_backward_char(&state.input, state.cursor);
                        }
                        Command::DeleteForwardChar => {
                            (state.input, state.cursor) =
                                delete_forward_char(&state.input, state.cursor);
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
                            (state.input, state.cursor) =
                                kill_backward_word(&state.input, state.cursor);
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
                            state.messages.push(Message::User { content: msg });
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
                        state.target_scroll_offset = state.target_scroll_offset.saturating_sub(3);
                    }
                    MouseEventKind::ScrollDown => {
                        let max = state
                            .total_rendered_lines
                            .saturating_sub(state.msg_area_height as usize)
                            as u16;
                        state.target_scroll_offset = (state.target_scroll_offset + 3).min(max);
                    }
                    _ => {}
                },
                Event::Resize(_, _) => {
                    let max = state
                        .total_rendered_lines
                        .saturating_sub(state.msg_area_height as usize)
                        as u16;
                    state.target_scroll_offset = state.target_scroll_offset.min(max);
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
                        state.messages.push(Message::Assistant {
                            content: String::new(),
                            thinking: None,
                            tool_calls: Vec::new(),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        if let Message::Assistant { content, .. } = &mut state.messages[idx] {
                            content.push_str(&token);
                        }
                    }
                }
                StreamEvent::Thinking(thinking) => {
                    if state.current_stream_message_idx.is_none() {
                        state.messages.push(Message::Assistant {
                            content: String::new(),
                            thinking: Some(String::new()),
                            tool_calls: Vec::new(),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        if let Message::Assistant { thinking: t, .. } = &mut state.messages[idx] {
                            if let Some(thinking_text) = t {
                                thinking_text.push_str(&thinking);
                            }
                        }
                    }
                }
                StreamEvent::ToolCall(tool_call) => {
                    if state.current_stream_message_idx.is_none() {
                        state.messages.push(Message::Assistant {
                            content: String::new(),
                            thinking: None,
                            tool_calls: Vec::new(),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        if let Message::Assistant { tool_calls, .. } = &mut state.messages[idx] {
                            tool_calls.push(tool_call);
                        }
                    }
                }
                StreamEvent::Done => {
                    state.current_stream_message_idx = None;

                    if let Some(Message::Assistant { tool_calls, .. }) = state.messages.last() {
                        if !tool_calls.is_empty() {
                            let tool_calls = tool_calls.clone();
                            let messages = state.messages.clone();
                            let tx = state.tx.clone();

                            tokio::spawn(async move {
                                let results = execute_tool_calls(tool_calls).await;
                                let mut updated_messages = messages;
                                for result in results {
                                    updated_messages.push(Message::ToolResult {
                                        tool_call_id: result.tool_call_id,
                                        content: result.content,
                                    });
                                }
                                let _ = stream_response(updated_messages, tx, port).await;
                            });

                            state.waiting = true;
                            state.spinner_idx = 0;
                        } else {
                            state.waiting = false;
                        }
                    } else {
                        state.waiting = false;
                    }
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
