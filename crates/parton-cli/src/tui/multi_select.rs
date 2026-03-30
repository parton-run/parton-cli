//! Ratatui multi-select widget — space to toggle, enter to confirm.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Run an interactive multi-select. Returns indices of selected items.
pub fn run_multi_select(title: &str, items: &[String]) -> io::Result<Vec<usize>> {
    let mut terminal = ratatui::init();
    let mut cursor = ListState::default();
    cursor.select(Some(0));
    let mut selected = vec![false; items.len()];

    loop {
        let selected_clone = selected.clone();
        terminal.draw(|frame| {
            render(frame, title, items, &selected_clone, &mut cursor);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = cursor.selected().unwrap_or(0);
                    cursor.select(Some(if i == 0 { items.len() - 1 } else { i - 1 }));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = cursor.selected().unwrap_or(0);
                    cursor.select(Some((i + 1) % items.len()));
                }
                KeyCode::Char(' ') => {
                    if let Some(i) = cursor.selected() {
                        selected[i] = !selected[i];
                    }
                }
                KeyCode::Enter => {
                    ratatui::restore();
                    let indices: Vec<usize> = selected
                        .iter()
                        .enumerate()
                        .filter(|(_, &s)| s)
                        .map(|(i, _)| i)
                        .collect();
                    return Ok(indices);
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    ratatui::restore();
                    return Ok(vec![]);
                }
                _ => {}
            }
        }
    }
}

fn render(
    frame: &mut Frame,
    title: &str,
    items: &[String],
    selected: &[bool],
    cursor: &mut ListState,
) {
    let selected_count = selected.iter().filter(|&&s| s).count();

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    // Header with count.
    let header = Paragraph::new(format!("{title} (selected: {selected_count})"))
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // List with checkboxes.
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let checkbox = if selected[i] { "◉" } else { "◯" };
            let style = if selected[i] {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(format!(" {checkbox} {label}"))).style(style)
        })
        .collect();

    let list = List::new(list_items)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯");

    frame.render_stateful_widget(list, chunks[1], cursor);

    // Footer.
    let footer = Paragraph::new("  ↑↓ navigate  Space toggle  Enter confirm  q cancel")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}
