use ratatui::prelude::{Constraint, Direction, Layout};
use ratatui::{DefaultTerminal, Frame};

fn main() -> anyhow::Result<()> {
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Fill(1), Constraint::Length(5)])
        .split(frame.area());

    frame.render_widget("hello world", layout[0]);
    frame.render_widget("what is up", layout[1]);
}
