//! Tab-based setup wizard using ratatui.
//!
//! Four tabs: Default | Planning | Execution | Judge
//! Each tab shows a selectable list of models. Tab/arrow keys to navigate.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

/// Labels for the four tabs.
const TAB_NAMES: [&str; 4] = ["Default", "Planning", "Execution", "Judge"];

/// State for the setup wizard.
pub struct WizardState {
    /// Currently active tab (0-3).
    active_tab: usize,
    /// Available model options (shared across tabs).
    models: Vec<String>,
    /// Selected model index per tab. None = inherit from Default.
    selections: [Option<usize>; 4],
    /// List scroll state per tab.
    list_states: [ListState; 4],
    /// Whether the wizard is complete.
    done: bool,
}

/// Result of the setup wizard.
pub struct WizardResult {
    /// Selected model index for each stage. None = use default.
    pub default: usize,
    pub planning: Option<usize>,
    pub execution: Option<usize>,
    pub judge: Option<usize>,
}

impl WizardState {
    /// Create a new wizard with the given model options.
    fn new(models: Vec<String>) -> Self {
        let mut default_list = ListState::default();
        default_list.select(Some(0));

        Self {
            active_tab: 0,
            models,
            selections: [Some(0), None, None, None],
            list_states: [
                default_list,
                ListState::default(),
                ListState::default(),
                ListState::default(),
            ],
            done: false,
        }
    }

    /// Build items for the current tab.
    fn tab_items(&self) -> Vec<String> {
        if self.active_tab == 0 {
            // Default tab: just show models.
            self.models.clone()
        } else {
            // Stage tabs: prepend "Use default" option.
            let default_idx = self.selections[0].unwrap_or(0);
            let default_name = self.models.get(default_idx).cloned().unwrap_or_default();
            let mut items = vec![format!("● Use default ({})", default_name)];
            items.extend(self.models.iter().cloned());
            items
        }
    }

    /// Get the effective selection for a stage tab (accounting for offset).
    fn effective_selection(&self, tab: usize) -> Option<usize> {
        if tab == 0 {
            self.selections[0]
        } else {
            match self.selections[tab] {
                None => None,           // Use default.
                Some(0) => None,        // "Use default" selected.
                Some(i) => Some(i - 1), // Offset by 1 for the "Use default" row.
            }
        }
    }
}

/// Run the tab-based setup wizard. Returns the selected model indices.
pub fn run_wizard(models: Vec<String>) -> io::Result<WizardResult> {
    let mut terminal = ratatui::init();
    let mut state = WizardState::new(models);

    loop {
        terminal.draw(|frame| render_wizard(frame, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                // Tab navigation.
                KeyCode::Tab | KeyCode::Right => {
                    state.active_tab = (state.active_tab + 1) % 4;
                    // Initialize list state if needed.
                    if state.list_states[state.active_tab].selected().is_none() {
                        state.list_states[state.active_tab].select(Some(0));
                    }
                }
                KeyCode::BackTab | KeyCode::Left => {
                    state.active_tab = if state.active_tab == 0 {
                        3
                    } else {
                        state.active_tab - 1
                    };
                    if state.list_states[state.active_tab].selected().is_none() {
                        state.list_states[state.active_tab].select(Some(0));
                    }
                }
                // List navigation.
                KeyCode::Up | KeyCode::Char('k') => {
                    let tab = state.active_tab;
                    let items_len = state.tab_items().len();
                    let i = state.list_states[tab].selected().unwrap_or(0);
                    state.list_states[tab].select(Some(if i == 0 { items_len - 1 } else { i - 1 }));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let tab = state.active_tab;
                    let items_len = state.tab_items().len();
                    let i = state.list_states[tab].selected().unwrap_or(0);
                    state.list_states[tab].select(Some((i + 1) % items_len));
                }
                // Select current item and advance to next tab (or save on last).
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let tab = state.active_tab;
                    let selected = state.list_states[tab].selected().unwrap_or(0);
                    state.selections[tab] = Some(selected);

                    if state.active_tab == 3 {
                        // Last tab — save and exit.
                        state.done = true;
                    } else {
                        // Advance to next tab.
                        state.active_tab += 1;
                        if state.list_states[state.active_tab].selected().is_none() {
                            state.list_states[state.active_tab].select(Some(0));
                        }
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    state.done = true;
                }
                _ => {}
            }
        }

        if state.done {
            ratatui::restore();
            return Ok(WizardResult {
                default: state.selections[0].unwrap_or(0),
                planning: state.effective_selection(1),
                execution: state.effective_selection(2),
                judge: state.effective_selection(3),
            });
        }
    }
}

/// Render the wizard UI.
fn render_wizard(frame: &mut Frame, state: &mut WizardState) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Title.
        Constraint::Length(3), // Tabs.
        Constraint::Min(0),    // Model list.
        Constraint::Length(3), // Summary.
        Constraint::Length(1), // Footer.
    ])
    .split(frame.area());

    // Title.
    let title = Paragraph::new("  Parton Setup — Model Configuration")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Tabs.
    let tab_titles: Vec<Line> = TAB_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let sel = state.effective_selection(i);
            let indicator = if i == 0 {
                "✓".to_string()
            } else {
                match sel {
                    Some(_) => "✓".to_string(),
                    None => "·".to_string(),
                }
            };
            Line::from(format!(" {indicator} {name} "))
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .select(state.active_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider("│")
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(tabs, chunks[1]);

    // Model list.
    let items = state.tab_items();
    let current_selection = state.selections[state.active_tab];

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let is_selected = current_selection == Some(i);
            let prefix = if is_selected { "● " } else { "  " };
            let style = if is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(format!("{prefix}{label}"))).style(style)
        })
        .collect();

    let list = List::new(list_items)
        .block(
            Block::default()
                .title(format!("  {} — Select model", TAB_NAMES[state.active_tab]))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ");

    frame.render_stateful_widget(list, chunks[2], &mut state.list_states[state.active_tab]);

    // Summary bar.
    let summary = build_summary(state);
    let summary_widget = Paragraph::new(summary)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title("  Summary"));
    frame.render_widget(summary_widget, chunks[3]);

    // Footer.
    let footer =
        Paragraph::new("  ←→/Tab switch stage  ↑↓ navigate  Enter select & next  q cancel")
            .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[4]);
}

/// Build the summary line showing selections across all stages.
fn build_summary(state: &WizardState) -> String {
    let default_name = state
        .models
        .get(state.selections[0].unwrap_or(0))
        .cloned()
        .unwrap_or_else(|| "?".into());

    let mut parts = vec![format!("Default: {default_name}")];

    for (i, name) in ["Plan", "Exec", "Judge"].iter().enumerate() {
        let stage = i + 1;
        let label = match state.effective_selection(stage) {
            Some(idx) => state.models.get(idx).cloned().unwrap_or_else(|| "?".into()),
            None => "default".into(),
        };
        parts.push(format!("{name}: {label}"));
    }

    format!("  {}", parts.join("  │  "))
}
