use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

pub enum Command {
    InsertChar(char),
    ClearInput,
    SubmitInput(String),
    Exit,
    None,
}

pub fn handle_user_input(key: KeyEvent, input: &str) -> Command {
    if key.kind == KeyEventKind::Press {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Command::ClearInput
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Command::Exit,
            KeyCode::Char(to_insert) => Command::InsertChar(to_insert),
            KeyCode::Enter => Command::SubmitInput(input.to_string()),
            KeyCode::Esc => Command::Exit,
            _ => Command::None,
        }
    } else {
        Command::None
    }
}
