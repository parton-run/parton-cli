//! Reusable ratatui selection widget — arrow keys to navigate, enter to confirm.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Run an interactive selection prompt. Returns the selected index.
///
/// Takes over the terminal, renders a list, and waits for user input.
pub fn run_select(title: &str, items: &[String], default: usize) -> io::Result<usize> {
    let mut terminal = ratatui::init();
    let mut state = ListState::default();
    state.select(Some(default));

    loop {
        terminal.draw(|frame| render_select(frame, title, items, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some(if i == 0 { items.len() - 1 } else { i - 1 }));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = state.selected().unwrap_or(0);
                    state.select(Some((i + 1) % items.len()));
                }
                KeyCode::Enter => {
                    ratatui::restore();
                    return Ok(state.selected().unwrap_or(0));
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    ratatui::restore();
                    return Ok(default);
                }
                _ => {}
            }
        }
    }
}

/// Render the selection list.
fn render_select(frame: &mut Frame, title: &str, items: &[String], state: &mut ListState) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    // Title.
    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // List.
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| ListItem::new(Line::from(s.as_str())))
        .collect();

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ");

    frame.render_stateful_widget(list, chunks[1], state);

    // Footer.
    let footer = Paragraph::new("  ↑↓ navigate  Enter confirm  q cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
