//! TUI clarification flow using ratatui select widget.

use parton_core::{ClarificationResult, PlanningContext, Question, QuestionType};

use super::{multi_select, select, style};

/// Run the interactive clarification TUI.
pub fn run_clarification(prompt: &str, result: &ClarificationResult) -> PlanningContext {
    let mut answers: Vec<(String, String)> = Vec::new();

    let skip =
        result.questions.is_empty() || (result.sufficient_for_planning && result.confidence >= 0.6);
    if skip {
        show_assumptions(&result.assumptions, result.confidence);
        return PlanningContext {
            intent: prompt.into(),
            mode: "auto".into(),
            answers,
            assumptions: result.assumptions.clone(),
            confidence: result.confidence,
        };
    }

    for question in &result.questions {
        if let Some(answer) = ask_question(question) {
            answers.push((question.question.clone(), answer));
        }
    }

    // Show collected answers.
    if !answers.is_empty() {
        eprintln!();
        style::print_header("Your Answers");
        for (question, answer) in &answers {
            style::print_ok(&format!("{question}: {answer}"));
        }
    }

    show_assumptions(&result.assumptions, result.confidence);

    PlanningContext {
        intent: prompt.into(),
        mode: "clarified".into(),
        answers,
        assumptions: result.assumptions.clone(),
        confidence: result.confidence,
    }
}

/// Ask a single question via ratatui selection.
fn ask_question(question: &Question) -> Option<String> {
    if question.options.is_empty() {
        return ask_free_text(&question.question);
    }

    match question.question_type {
        QuestionType::SingleSelect => {
            let idx = select::run_select(&question.question, &question.options, 0).ok()?;

            let chosen = &question.options[idx];
            if chosen.starts_with("Other") {
                ask_free_text(&format!("{} (your answer)", question.question))
            } else {
                Some(chosen.clone())
            }
        }
        QuestionType::MultiSelect => {
            let indices =
                multi_select::run_multi_select(&question.question, &question.options).ok()?;

            let mut chosen: Vec<String> = indices
                .iter()
                .map(|&i| question.options[i].clone())
                .collect();

            // If "Other" was selected, ask for custom input.
            if let Some(pos) = chosen.iter().position(|s| s.starts_with("Other")) {
                if let Some(custom) = ask_free_text(&format!("{} (your answer)", question.question))
                {
                    chosen[pos] = custom;
                } else {
                    chosen.remove(pos);
                }
            }

            if chosen.is_empty() {
                None
            } else {
                Some(chosen.join(", "))
            }
        }
    }
}

/// Free text input (falls back to stdio since ratatui doesn't have text input).
fn ask_free_text(prompt: &str) -> Option<String> {
    use std::io::{self, Write};

    eprint!("  {}: ", prompt);
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Display assumptions and confidence after clarification.
fn show_assumptions(assumptions: &[String], confidence: f32) {
    if assumptions.is_empty() {
        return;
    }

    eprintln!();
    style::print_header("AI Assumptions");
    for a in assumptions {
        eprintln!("  • {a}");
    }

    let pct = (confidence * 100.0) as u32;
    let label = match pct {
        80..=100 => "High confidence",
        50..=79 => "Moderate confidence",
        _ => "Low confidence",
    };
    eprintln!();
    style::print_kv("Confidence", &format!("{pct}% — {label}"));
}
