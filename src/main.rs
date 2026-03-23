use crossterm::event::{self, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::{Constraint, Direction, Layout};
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Padding};
use ratatui::{DefaultTerminal, Frame};

const HORIZONTAL_MARGIN: u16 = 2;
const VERTICAL_MARGIN: u16 = 1;
const SPACING: u16 = 1;

enum InputCommand {
    InsertChar(char),
    ClearInput,
    SubmitInput(String),
    Exit,
    None,
}

fn main() -> anyhow::Result<()> {
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let mut input = String::new();
    let mut messages: Vec<String> = Vec::new();

    loop {
        terminal.draw(|frame| render(frame, &input))?;

        if let Some(key) = event::read()?.as_key_press_event() {
            match handle_user_input(key, &input) {
                InputCommand::Exit => break Ok(()),
                InputCommand::InsertChar(ch) => insert(&mut input, ch),
                InputCommand::ClearInput => input.clear(),
                InputCommand::SubmitInput(msg) => {
                    messages.push(msg);
                    input.clear();
                }
                InputCommand::None => {}
            }
        }
    }
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

fn render(frame: &mut Frame, input: &str) {
    let layout = Layout::default()
        .vertical_margin(VERTICAL_MARGIN)
        .horizontal_margin(HORIZONTAL_MARGIN)
        .spacing(SPACING)
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Fill(1),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let input_block = Block::new()
        .white()
        .on_black()
        .padding(Padding::new(2, 2, 1, 1));
    let inner = input_block.inner(layout[1]);

    frame.render_widget("hello world", layout[0]);
    frame.render_widget(input_block, layout[1]);
    frame.render_widget(input, inner);
    frame.render_widget("status", layout[2]);
}

fn insert(input: &mut String, ch: char) {
    input.push(ch);
}
