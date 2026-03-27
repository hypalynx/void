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
use void::render::render_message;
use void::stream::{StreamEvent, stream_response};
use void::tool;
use void::types::{AppState, ApiMessage, DisplayMessage, DisplayRole, ToolCall, ToolResultInfo};
use void::ui::{draw, spinner_len};
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Modifier};

#[derive(Parser)]
#[command(name = "void")]
struct Cli {
    #[arg(short, long, default_value = "7777")]
    port: u16,
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    api_key: Option<String>,
    #[arg(long)]
    path_prefix: Option<String>,
    #[arg(long)]
    profile: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config file
    let config = void::config::load();

    // Determine profile name: CLI flag > default in config > "local"
    let profile_name = cli.profile
        .or_else(|| void::config::get_default_profile_name(&config))
        .unwrap_or_else(|| "local".to_string());

    // Load the profile
    let profile = void::config::get_profile(&config, &profile_name)
        .unwrap_or_default();

    // Merge values: profile defaults < CLI flags override
    let host = cli.host
        .or_else(|| profile.host.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let port = if cli.port != 7777 {
        cli.port
    } else {
        profile.port.unwrap_or(7777)
    };

    let model = cli.model.or_else(|| profile.model.clone());
    let path_prefix = cli.path_prefix.or_else(|| profile.path_prefix.clone());

    // API key: CLI flag > VOID_API_KEY env var > profile's api_key_env
    let api_key = cli.api_key
        .or_else(|| std::env::var("VOID_API_KEY").ok())
        .or_else(|| {
            profile.api_key_env.as_ref()
                .and_then(|env_var| std::env::var(env_var).ok())
        });

    let mut terminal = ratatui::init();
    execute!(io::stdout(), crossterm::event::EnableMouseCapture)?;
    let result = app(
        &mut terminal,
        port,
        host,
        model,
        api_key,
        path_prefix,
    )
    .await;
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

            let (content, diff) = match tool::execute(&tool_obj) {
                Ok(output) => (output.content, output.diff),
                Err(e) => (format!("Tool execution error: {}", e), None),
            };

            let tool_name = tool_call.function.name.clone();
            let tool_args = tool_call.function.arguments.clone();

            ToolResultInfo {
                tool_call_id: tool_call.id,
                tool_name,
                tool_args,
                content,
                diff,
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

async fn app(
    terminal: &mut DefaultTerminal,
    port: u16,
    host: String,
    model: Option<String>,
    api_key: Option<String>,
    path_prefix: Option<String>,
) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut state = AppState {
        input: String::new(),
        cursor: 0,
        clipboard: String::new(),
        messages: Vec::new(),
        api_log: Vec::new(),
        tool_status: Vec::new(),
        show_tool_detail: false,
        port,
        host,
        model,
        api_key,
        path_prefix,
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
        input_history: Vec::new(),
        history_idx: None,
        history_draft: String::new(),
    };

    // Load input history from ~/.void/history
    if let Ok(home) = std::env::var("HOME") {
        let history_path = format!("{}/.void/history", home);
        if let Ok(contents) = std::fs::read_to_string(&history_path) {
            state.input_history = contents
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
        }
    }

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

        // Capture whether user was at bottom before draw updates total_rendered_lines
        let old_max = state.total_rendered_lines.saturating_sub(state.msg_area_height as usize) as u16;
        let was_at_bottom = state.target_scroll_offset >= old_max;

        let draw_start = Instant::now();
        terminal.draw(|frame| draw(frame, &mut state))?;
        state.last_draw_ms = draw_start.elapsed().as_secs_f64() * 1000.0;

        // Sticky bottom: if user was at bottom, track new bottom (fixes both issues)
        if was_at_bottom {
            let new_max = state.total_rendered_lines.saturating_sub(state.msg_area_height as usize) as u16;
            state.target_scroll_offset = new_max;
        }

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
                            state.cursor = move_start_of_line(&state.input, state.cursor);
                        }
                        Command::MoveEndOfLine => {
                            state.cursor = move_end_of_line(&state.input, state.cursor);
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
                            state.messages.push(DisplayMessage {
                                role: DisplayRole::User,
                                content: msg.clone(),
                                thinking: None,
                                detail: None,
                                diff: None,
                                lines: Vec::new(),
                                thinking_lines: Vec::new(),
                                detail_lines: Vec::new(),
                            });
                            state.api_log.push(ApiMessage::User { content: msg.clone() });

                            // Save to history and reset index
                            if !msg.is_empty() {
                                state.input_history.push(msg.clone());
                                state.history_idx = None;
                                // Append to ~/.void/history
                                if let Ok(home) = std::env::var("HOME") {
                                    let history_path = format!("{}/.void/history", home);
                                    let _ = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(&history_path)
                                        .and_then(|mut f| {
                                            use std::io::Write;
                                            writeln!(f, "{}", msg)
                                        });
                                }
                            }

                            state.input.clear();
                            state.cursor = 0;
                            state.waiting = true;
                            state.spinner_idx = 0;

                            let api_log = state.api_log.clone();
                            let tx = state.tx.clone();
                            let port = state.port;
                            let host = state.host.clone();
                            let model = state.model.clone();
                            let api_key = state.api_key.clone();
                            let path_prefix = state.path_prefix.clone();
                            tokio::spawn(async move {
                                let _ = stream_response(api_log, tx, port, host, model, api_key, path_prefix).await;
                            });
                        }
                        Command::ToggleToolDetail => {
                            state.show_tool_detail = !state.show_tool_detail;
                        }
                        Command::NewLine => {
                            state.input.insert(state.cursor, '\n');
                            state.cursor += 1;
                            state.history_idx = None;
                        }
                        Command::MoveLineUp => {
                            use void::input::{is_first_line, cursor_up};
                            if is_first_line(&state.input, state.cursor) {
                                if !state.input_history.is_empty() {
                                    // Save current draft when first entering history
                                    if state.history_idx.is_none() {
                                        state.history_draft = state.input.clone();
                                    }
                                    let idx = state.history_idx.map(|i| i.saturating_sub(1))
                                        .unwrap_or(state.input_history.len() - 1);
                                    state.history_idx = Some(idx);
                                    state.input = state.input_history[idx].clone();
                                    state.cursor = state.input.len();
                                }
                            } else {
                                state.cursor = cursor_up(&state.input, state.cursor);
                            }
                        }
                        Command::MoveLineDown => {
                            use void::input::{is_last_line, cursor_down};
                            if is_last_line(&state.input, state.cursor) {
                                match state.history_idx {
                                    None => {}
                                    Some(i) if i + 1 >= state.input_history.len() => {
                                        // Returning to present — restore draft
                                        state.history_idx = None;
                                        state.input = state.history_draft.clone();
                                        state.cursor = state.input.len();
                                    }
                                    Some(i) => {
                                        let idx = i + 1;
                                        state.history_idx = Some(idx);
                                        state.input = state.input_history[idx].clone();
                                        state.cursor = state.input.len();
                                    }
                                }
                            } else {
                                state.cursor = cursor_down(&state.input, state.cursor);
                            }
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
                        state.messages.push(DisplayMessage {
                            role: DisplayRole::Assistant,
                            content: String::new(),
                            thinking: None,
                            detail: None,
                            diff: None,
                            lines: Vec::new(),
                            thinking_lines: Vec::new(),
                            detail_lines: Vec::new(),
                        });
                        state.api_log.push(ApiMessage::Assistant {
                            content: String::new(),
                            thinking: None,
                            tool_calls: Vec::new(),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        state.messages[idx].content.push_str(&token);
                        if let ApiMessage::Assistant { content, .. } = &mut state.api_log[idx] {
                            content.push_str(&token);
                        }
                    }
                }
                StreamEvent::Thinking(thinking) => {
                    if state.current_stream_message_idx.is_none() {
                        state.messages.push(DisplayMessage {
                            role: DisplayRole::Assistant,
                            content: String::new(),
                            thinking: Some(String::new()),
                            detail: None,
                            diff: None,
                            lines: Vec::new(),
                            thinking_lines: Vec::new(),
                            detail_lines: Vec::new(),
                        });
                        state.api_log.push(ApiMessage::Assistant {
                            content: String::new(),
                            thinking: Some(String::new()),
                            tool_calls: Vec::new(),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        // Append to both display and api messages
                        if let Some(thinking_text) = &mut state.messages[idx].thinking {
                            thinking_text.push_str(&thinking);
                        }
                        if let ApiMessage::Assistant { thinking: t, .. } = &mut state.api_log[idx] {
                            if let Some(thinking_text) = t {
                                thinking_text.push_str(&thinking);
                            }
                        }
                    }
                }
                StreamEvent::ToolCall(tool_call) => {
                    if state.current_stream_message_idx.is_none() {
                        state.messages.push(DisplayMessage {
                            role: DisplayRole::Assistant,
                            content: String::new(),
                            thinking: None,
                            detail: None,
                            diff: None,
                            lines: Vec::new(),
                            thinking_lines: Vec::new(),
                            detail_lines: Vec::new(),
                        });
                        state.api_log.push(ApiMessage::Assistant {
                            content: String::new(),
                            thinking: None,
                            tool_calls: Vec::new(),
                        });
                        state.current_stream_message_idx = Some(state.messages.len() - 1);
                    }
                    if let Some(idx) = state.current_stream_message_idx {
                        if let ApiMessage::Assistant { tool_calls, .. } = &mut state.api_log[idx] {
                            tool_calls.push(tool_call);
                        }
                    }
                }
                StreamEvent::Done => {
                    state.current_stream_message_idx = None;

                    // Cache rendered lines for the completed assistant message
                    if let Some(msg) = state.messages.last_mut() {
                        if msg.role == DisplayRole::Assistant {
                            if msg.lines.is_empty() {
                                msg.lines = render_message(&msg.content)
                                    .into_iter()
                                    .map(|spans| Line::from(spans))
                                    .collect();
                            }
                            if msg.thinking_lines.is_empty() {
                                if let Some(thinking) = &msg.thinking {
                                    msg.thinking_lines = render_message(thinking)
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
                                        .collect();
                                }
                            }
                        }
                    }

                    if let Some(ApiMessage::Assistant { tool_calls, .. }) = state.api_log.last() {
                        if !tool_calls.is_empty() {
                            let tool_calls = tool_calls.clone();
                            let tx = state.tx.clone();

                            tokio::spawn(async move {
                                // Emit tool execution started events
                                for tool_call in &tool_calls {
                                    let desc = format!("Executing {}...", tool_call.function.name);
                                    let _ = tx.send(StreamEvent::ToolExecuting(desc));
                                }

                                let results = execute_tool_calls(tool_calls).await;
                                let _ = tx.send(StreamEvent::ToolsExecuted(results));
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
                StreamEvent::ToolExecuting(desc) => {
                    state.tool_status.push(desc);
                }
                StreamEvent::ToolsExecuted(results) => {
                    state.tool_status.clear();

                    for result in results {
                        // Add to api_log
                        state.api_log.push(ApiMessage::ToolResult {
                            tool_call_id: result.tool_call_id.clone(),
                            content: result.content.clone(),
                        });

                        // Add permanent record to messages
                        let args_map = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                            &result.tool_args,
                        ).unwrap_or_default();
                        let call_label = tool::format_tool_call(&result.tool_name, &args_map);
                        let summary = result.content.clone();
                        state.messages.push(DisplayMessage {
                            role: DisplayRole::ToolActivity,
                            content: call_label,
                            thinking: None,
                            detail: Some(summary),
                            diff: result.diff.clone(),
                            lines: Vec::new(),
                            thinking_lines: Vec::new(),
                            detail_lines: Vec::new(),
                        });
                    }

                    // Re-invoke LLM
                    let api_log = state.api_log.clone();
                    let tx = state.tx.clone();
                    let port = state.port;
                    let host = state.host.clone();
                    let model = state.model.clone();
                    let api_key = state.api_key.clone();
                    let path_prefix = state.path_prefix.clone();
                    tokio::spawn(async move {
                        let _ = stream_response(api_log, tx, port, host, model, api_key, path_prefix).await;
                    });

                    state.waiting = true;
                    state.spinner_idx = 0;
                }
                StreamEvent::Error(error_msg) => {
                    state.messages.push(DisplayMessage {
                        role: DisplayRole::ToolActivity,
                        content: format!("❌ API Error: {}", error_msg),
                        thinking: None,
                        detail: None,
                        diff: None,
                        lines: Vec::new(),
                        thinking_lines: Vec::new(),
                        detail_lines: Vec::new(),
                    });
                    state.waiting = false;
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
