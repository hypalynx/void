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
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::ToggleToolDetail
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Command::Exit,
            KeyCode::Left => Command::MoveBackwardChar,
            KeyCode::Right => Command::MoveForwardChar,
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

pub fn move_start_of_line() -> usize {
    0
}

pub fn move_end_of_line(input: &str) -> usize {
    input.len()
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
    let killed = input[..cursor].to_string();
    let remaining = input[cursor..].to_string();
    (remaining, 0, killed)
}

pub fn yank(input: &str, cursor: usize, clipboard: &str) -> (String, usize) {
    let mut result = input.to_string();
    result.insert_str(cursor, clipboard);
    (result, cursor + clipboard.len())
}
