//! Interactive plan review screen with comments and replan.
//!
//! Fullscreen ratatui view: scroll through files, add per-file or general comments.
//! Enter approves (if no comments) or triggers replan (if comments exist).

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use parton_core::{FileAction, RunPlan};

/// A review comment attached to the plan or a specific file.
#[derive(Clone, Debug)]
pub struct ReviewComment {
    /// None = general comment, Some(path) = file-specific.
    pub file: Option<String>,
    /// Comment text.
    pub text: String,
}

/// Result of the plan review.
pub enum ReviewDecision {
    /// Plan approved as-is (no comments).
    Approve,
    /// Replan with these comments.
    Replan(Vec<ReviewComment>),
    /// User rejected the plan entirely.
    Reject,
}

/// State for the plan review screen.
struct ReviewState {
    plan: RunPlan,
    list_state: ListState,
    comments: Vec<ReviewComment>,
    mode: Mode,
    input_buffer: String,
    /// Which file is being commented (None = general).
    comment_target: Option<String>,
    done: Option<ReviewDecision>,
}

#[derive(PartialEq)]
enum Mode {
    /// Browsing the file list.
    Browse,
    /// Typing a comment.
    Input,
}

impl ReviewState {
    fn new(plan: RunPlan) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            plan,
            list_state,
            comments: Vec::new(),
            mode: Mode::Browse,
            input_buffer: String::new(),
            comment_target: None,
            done: None,
        }
    }

    fn selected_file(&self) -> Option<&str> {
        let idx = self.list_state.selected()?;
        self.plan.files.get(idx).map(|f| f.path.as_str())
    }

    fn file_comments(&self, path: &str) -> Vec<&ReviewComment> {
        self.comments
            .iter()
            .filter(|c| c.file.as_deref() == Some(path))
            .collect()
    }

    fn general_comments(&self) -> Vec<&ReviewComment> {
        self.comments.iter().filter(|c| c.file.is_none()).collect()
    }
}

/// Run the interactive plan review. Returns the user's decision.
pub fn run_review(plan: RunPlan) -> io::Result<ReviewDecision> {
    let mut terminal = ratatui::init();
    let mut state = ReviewState::new(plan);

    loop {
        terminal.draw(|frame| render_review(frame, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match state.mode {
                Mode::Browse => handle_browse_key(&mut state, key.code),
                Mode::Input => handle_input_key(&mut state, key.code),
            }
        }

        if let Some(decision) = state.done.take() {
            ratatui::restore();
            return Ok(decision);
        }
    }
}

fn handle_browse_key(state: &mut ReviewState, code: KeyCode) {
    let file_count = state.plan.files.len();

    match code {
        KeyCode::Up | KeyCode::Char('k') => {
            let i = state.list_state.selected().unwrap_or(0);
            state.list_state.select(Some(if i == 0 { file_count - 1 } else { i - 1 }));
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let i = state.list_state.selected().unwrap_or(0);
            state.list_state.select(Some((i + 1) % file_count));
        }
        // Comment on selected file.
        KeyCode::Char('c') => {
            state.comment_target = state.selected_file().map(String::from);
            state.input_buffer.clear();
            state.mode = Mode::Input;
        }
        // General comment.
        KeyCode::Char('g') => {
            state.comment_target = None;
            state.input_buffer.clear();
            state.mode = Mode::Input;
        }
        // Enter: approve if no comments, replan if comments.
        KeyCode::Enter => {
            if state.comments.is_empty() {
                state.done = Some(ReviewDecision::Approve);
            } else {
                state.done = Some(ReviewDecision::Replan(state.comments.clone()));
            }
        }
        // Reject.
        KeyCode::Char('r') | KeyCode::Esc => {
            state.done = Some(ReviewDecision::Reject);
        }
        _ => {}
    }
}

fn handle_input_key(state: &mut ReviewState, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            let text = state.input_buffer.trim().to_string();
            if !text.is_empty() {
                state.comments.push(ReviewComment {
                    file: state.comment_target.clone(),
                    text,
                });
            }
            state.input_buffer.clear();
            state.mode = Mode::Browse;
        }
        KeyCode::Esc => {
            state.input_buffer.clear();
            state.mode = Mode::Browse;
        }
        KeyCode::Backspace => {
            state.input_buffer.pop();
        }
        KeyCode::Char(ch) => {
            state.input_buffer.push(ch);
        }
        _ => {}
    }
}

fn render_review(frame: &mut Frame, state: &mut ReviewState) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header.
        Constraint::Min(0),   // File list + detail.
        Constraint::Length(5), // Comments.
        Constraint::Length(3), // Input or footer.
    ])
    .split(frame.area());

    // Header.
    let comment_count = state.comments.len();
    let enter_action = if comment_count == 0 { "approve" } else { "replan" };
    let header_text = format!(
        "  Plan Review — {} files  │  {} comments  │  Enter = {}",
        state.plan.files.len(),
        comment_count,
        enter_action,
    );
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Split middle: file list (left) + detail (right).
    let mid = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    // File list.
    let list_items: Vec<ListItem> = state
        .plan
        .files
        .iter()
        .map(|f| {
            let action = match f.action {
                FileAction::Create => "+",
                FileAction::Edit => "~",
            };
            let comment_marker = if state.file_comments(&f.path).is_empty() {
                " "
            } else {
                "💬"
            };
            ListItem::new(Line::from(format!(" {action} {comment_marker} {}", f.path)))
        })
        .collect();

    let list = List::new(list_items)
        .block(Block::default().title("  Files").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("❯");

    frame.render_stateful_widget(list, mid[0], &mut state.list_state);

    // File detail.
    let detail_text = if let Some(idx) = state.list_state.selected() {
        let file = &state.plan.files[idx];
        let mut lines = vec![
            format!("Path: {}", file.path),
            format!("Action: {:?}", file.action),
            String::new(),
            "Goal:".into(),
            file.goal.clone(),
        ];
        if !file.must_export.is_empty() {
            lines.push(String::new());
            lines.push(format!("Exports: {}", file.must_export.join(", ")));
        }
        if !file.must_import_from.is_empty() {
            lines.push(String::new());
            for imp in &file.must_import_from {
                lines.push(format!("Import from {}: {}", imp.path, imp.symbols.join(", ")));
            }
        }
        lines.join("\n")
    } else {
        "Select a file to see details".into()
    };

    let detail = Paragraph::new(detail_text)
        .block(Block::default().title("  Detail").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, mid[1]);

    // Comments section.
    let comment_lines: Vec<Line> = state
        .comments
        .iter()
        .map(|c| {
            let target = c.file.as_deref().unwrap_or("GENERAL");
            Line::from(format!("  [{target}] {}", c.text))
        })
        .collect();

    let general_marker = if state.general_comments().is_empty() { "" } else { " 💬" };
    let comments = Paragraph::new(comment_lines)
        .block(
            Block::default()
                .title(format!("  Comments ({comment_count}){general_marker}"))
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(comments, chunks[2]);

    // Input bar or footer.
    if state.mode == Mode::Input {
        let target = state
            .comment_target
            .as_deref()
            .unwrap_or("general plan");
        let input = Paragraph::new(format!("  Comment on {target}: {}▌", state.input_buffer))
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(input, chunks[3]);
    } else {
        let footer_text = if comment_count == 0 {
            "  ↑↓ navigate  c file comment  g general comment  Enter approve  r reject"
        } else {
            "  ↑↓ navigate  c file comment  g general comment  Enter replan with comments  r reject"
        };
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::TOP));
        frame.render_widget(footer, chunks[3]);
    }
}
