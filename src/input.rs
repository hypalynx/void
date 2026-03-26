use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub enum Command {
    InsertChar(char),
    ClearInput,
    SubmitInput(String),
    KillBackwardWord,
    DeleteBackwardChar,
    DeleteForwardChar,
    MoveBackwardChar,
    MoveForwardChar,
    MoveBackwardWord,
    MoveForwardWord,
    MoveStartOfLine,
    MoveEndOfLine,
    KillBackwardLine,
    Yank,
    ToggleToolDetail,
    NewLine,
    MoveLineUp,
    MoveLineDown,
    Exit,
    None,
}

pub fn handle_user_input(key: KeyEvent, _input: &str) -> Command {
    if key.kind == KeyEventKind::Press {
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::MoveStartOfLine
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::MoveEndOfLine
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::MoveBackwardChar
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::MoveForwardChar
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                Command::MoveBackwardWord
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                Command::MoveForwardWord
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::DeleteForwardChar
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::KillBackwardLine
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::KillBackwardWord
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::Yank
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::NewLine
            }
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::ToggleToolDetail
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Command::Exit,
            KeyCode::Left => Command::MoveBackwardChar,
            KeyCode::Right => Command::MoveForwardChar,
            KeyCode::Up => Command::MoveLineUp,
            KeyCode::Down => Command::MoveLineDown,
            KeyCode::Backspace => Command::DeleteBackwardChar,
            KeyCode::Char(to_insert) => Command::InsertChar(to_insert),
            KeyCode::Enter => Command::SubmitInput(_input.to_string()),
            KeyCode::Esc => Command::Exit,
            _ => Command::None,
        }
    } else {
        Command::None
    }
}

pub fn delete_backward_char(input: &str, cursor: usize) -> (String, usize) {
    if cursor == 0 {
        return (input.to_string(), 0);
    }
    let mut result = input.to_string();
    let new_cursor = cursor - 1;
    result.remove(new_cursor);
    (result, new_cursor)
}

pub fn delete_forward_char(input: &str, cursor: usize) -> (String, usize) {
    if cursor >= input.len() {
        return (input.to_string(), cursor);
    }
    let mut result = input.to_string();
    result.remove(cursor);
    (result, cursor)
}

pub fn move_backward_char(cursor: usize) -> usize {
    cursor.saturating_sub(1)
}

pub fn move_forward_char(input: &str, cursor: usize) -> usize {
    (cursor + 1).min(input.len())
}

pub fn move_start_of_line(input: &str, cursor: usize) -> usize {
    // Find last \n before cursor, or 0 if none
    input[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

pub fn move_end_of_line(input: &str, cursor: usize) -> usize {
    // Find next \n after cursor, or end of input
    input[cursor..].find('\n').map(|i| cursor + i).unwrap_or(input.len())
}

pub fn move_backward_word(input: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }

    let chars: Vec<char> = input.chars().collect();
    let mut pos = cursor;

    // Skip whitespace backward
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    // Skip word characters backward
    while pos > 0 && !chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    pos
}

pub fn move_forward_word(input: &str, cursor: usize) -> usize {
    if cursor >= input.len() {
        return input.len();
    }

    let chars: Vec<char> = input.chars().collect();
    let mut pos = cursor;

    // Skip word characters forward
    while pos < chars.len() && !chars[pos].is_whitespace() {
        pos += 1;
    }

    // Skip whitespace forward
    while pos < chars.len() && chars[pos].is_whitespace() {
        pos += 1;
    }

    pos
}

pub fn kill_backward_word(input: &str, cursor: usize) -> (String, usize) {
    if cursor == 0 {
        return (input.to_string(), 0);
    }

    let chars: Vec<char> = input.chars().collect();
    let mut end = cursor;

    // Skip trailing whitespace
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }

    // Delete the word (until we hit whitespace or start of string)
    while end > 0 && !chars[end - 1].is_whitespace() {
        end -= 1;
    }

    let result = chars[..end].iter().collect::<String>() + &chars[cursor..].iter().collect::<String>();
    (result, end)
}

pub fn kill_backward_line(input: &str, cursor: usize) -> (String, usize, String) {
    // Find start of current line
    let line_start = input[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let killed = input[line_start..cursor].to_string();
    let mut result = input[..line_start].to_string();
    result.push_str(&input[cursor..]);
    (result, line_start, killed)
}

pub fn yank(input: &str, cursor: usize, clipboard: &str) -> (String, usize) {
    let mut result = input.to_string();
    result.insert_str(cursor, clipboard);
    (result, cursor + clipboard.len())
}

/// Check if cursor is on the first line of input
pub fn is_first_line(input: &str, cursor: usize) -> bool {
    !input[..cursor].contains('\n')
}

/// Check if cursor is on the last line of input
pub fn is_last_line(input: &str, cursor: usize) -> bool {
    !input[cursor..].contains('\n')
}

/// Move cursor up one line in multiline input
pub fn cursor_up(input: &str, cursor: usize) -> usize {
    // Find start of current line
    let line_start = input[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = cursor - line_start;

    // Already on first line
    if line_start == 0 {
        return cursor;
    }

    // Find start of previous line
    let prev_line_end = line_start - 1; // position of \n
    let prev_line_start = input[..prev_line_end].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let prev_line_len = prev_line_end - prev_line_start;

    prev_line_start + col.min(prev_line_len)
}

/// Move cursor down one line in multiline input
pub fn cursor_down(input: &str, cursor: usize) -> usize {
    // Find start of next line
    if let Some(nl_pos) = input[cursor..].find('\n') {
        let next_line_start = cursor + nl_pos + 1;

        // Find current line start to compute column
        let line_start = input[..cursor].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = cursor - line_start;

        // Find end of next line
        let next_line_end = input[next_line_start..].find('\n')
            .map(|i| next_line_start + i)
            .unwrap_or(input.len());
        let next_line_len = next_line_end - next_line_start;

        next_line_start + col.min(next_line_len)
    } else {
        cursor // already on last line
    }
}
