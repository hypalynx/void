use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::{Constraint, Direction, Layout};
use ratatui::style::Stylize;
use ratatui::widgets::Block;
use ratatui::{DefaultTerminal, Frame};

fn main() -> anyhow::Result<()> {
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let mut input = String::new();

    loop {
        terminal.draw(|frame| render(frame, &input))?;
        // TODO enum to control whether input is active

        if let Some(key) = event::read()?.as_key_press_event() {
            if handle_user_input(key, &mut input) {
                break Ok(());
            }
        }
    }
}

// TODO instead of bool we probably want to return some kind of state change,
// currently bool is just meant to say whether to exit the loop, stopping the program.
fn handle_user_input(key: KeyEvent, input: &mut String) -> bool {
    if key.kind == KeyEventKind::Press {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.clear();
                false
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
            KeyCode::Char(to_insert) => {
                insert(input, to_insert);
                false
            }
            KeyCode::Esc => true,
            _ => false,
        }
    } else {
        false
    }
}

fn render(frame: &mut Frame, input: &str) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Length(5)])
        .split(frame.area());

    let input_block = Block::new().white().on_black();

    frame.render_widget("hello world", layout[0]);
    frame.render_widget(input_block, layout[1]);
    frame.render_widget(input, layout[1]);
}

fn insert(input: &mut String, ch: char) {
    input.push(ch);
}
