//! Clarification question generation via LLM.
//!
//! Generates targeted questions to fill in missing details before planning.
//! Prompts are battle-tested from production Parton.

use parton_core::{
    ClarificationResult, ModelProvider, ProviderError, Question, QuestionType, ToolCall,
    ToolDefinition, ToolResult,
};

/// Generate clarification questions for a user prompt.
pub async fn generate_questions(
    prompt: &str,
    is_greenfield: bool,
    graph_summary: &str,
    provider: &dyn ModelProvider,
) -> Result<ClarificationResult, ProviderError> {
    generate_questions_inner(prompt, is_greenfield, graph_summary, provider, &[], None).await
}

/// Generate clarification questions with tool-use support.
///
/// When tools are provided, the LLM can inspect the codebase to
/// avoid asking about things visible in the code.
pub async fn generate_questions_with_tools(
    prompt: &str,
    is_greenfield: bool,
    graph_summary: &str,
    provider: &dyn ModelProvider,
    tools: &[ToolDefinition],
    handle_tool: &(dyn Fn(ToolCall) -> ToolResult + Send + Sync),
) -> Result<ClarificationResult, ProviderError> {
    generate_questions_inner(
        prompt,
        is_greenfield,
        graph_summary,
        provider,
        tools,
        Some(handle_tool),
    )
    .await
}

/// Shared implementation for question generation.
async fn generate_questions_inner(
    prompt: &str,
    is_greenfield: bool,
    graph_summary: &str,
    provider: &dyn ModelProvider,
    tools: &[ToolDefinition],
    handle_tool: Option<&(dyn Fn(ToolCall) -> ToolResult + Send + Sync)>,
) -> Result<ClarificationResult, ProviderError> {
    let system = build_system_prompt(is_greenfield);
    let repo_ctx = if is_greenfield {
        "New project with no existing code.".to_string()
    } else if graph_summary.is_empty() {
        "Existing project with code already in place.".to_string()
    } else {
        format!("Existing project with code already in place.\n\n{graph_summary}")
    };
    let user = format!(
        "Repository context: {repo_ctx}\n\nUser's request:\n{prompt}\n\n\
         Generate clarification questions to fill in the gaps needed for implementation planning.",
    );

    let response = if let (false, Some(handler)) = (tools.is_empty(), handle_tool) {
        provider
            .send_with_tools(&system, &user, tools, 5, handler)
            .await?
    } else {
        provider.send(&system, &user, true).await?
    };

    let mut result = parse_response(&response.content)?;
    add_other_option(&mut result.questions);
    Ok(result)
}

fn build_system_prompt(is_greenfield: bool) -> String {
    let priority = if is_greenfield {
        r#"This is a GREENFIELD project (new, no existing code). Prioritize questions about:
1. Application type (web app, CLI tool, API server, library, etc.)
2. Framework preferences (e.g., React, Express, Actix, etc.)
3. Programming language (if not already clear)
4. Data persistence needs (database type, ORM, etc.)
5. Authentication requirements
6. Styling/UI approach (CSS framework, design system, etc.)"#
    } else {
        r#"This is an EXISTING project with code already in place. Prioritize questions about:
1. Scope of the change (which parts of the codebase are affected)
2. Expected behavior (inputs, outputs, edge cases)
3. Constraints (backwards compatibility, performance requirements, etc.)
4. Ambiguous BUSINESS requirements that cannot be inferred from code

NEVER ask about things visible in the Existing Code section:
- How auth/admin access works — READ the key file snippets
- What database/ORM is used — READ the schema files
- What framework/libraries are used — READ the imports
- How existing components work — READ their exports and signatures
If the answer is in the code, state it as an assumption, don't ask."#
    };

    format!(
        r#"You are a clarification engine for a code generation system.
Your job is to generate targeted questions that gather missing information needed to plan an implementation.

{priority}

RULES:
- ALL output MUST be in English — questions, options, assumptions, everything.
- Generate at most 5 questions
- Each question must have a clear reason explaining why it is being asked
- Use "SingleSelect" for questions with a fixed set of options
- Use "MultiSelect" when the user may pick more than one option
- NEVER use "FreeText" as the question type. Always use "SingleSelect" or "MultiSelect" with concrete options. Always include an "Other (please specify)" option so the user can type a custom answer if none of the options fit.
- If you determine there is already enough information to proceed with planning, set "sufficient_for_planning" to true and return fewer or zero questions
- Every question must have a unique "id" (e.g., "q1", "q2", etc.)
- Do NOT ask about things that are already known from the repository context
- NEVER generate assumptions about testing or versions — the system handles those automatically

You MUST respond with ONLY a JSON object (no markdown fences, no explanation) matching this schema:
{{
  "questions": [
    {{
      "id": "q1",
      "question_type": "SingleSelect" | "MultiSelect",
      "question": "The question text",
      "options": ["option1", "option2"],
      "reason": "Why this question is being asked"
    }}
  ],
  "assumptions": ["assumption1", "assumption2"],
  "confidence": 0.0 to 1.0,
  "sufficient_for_planning": true | false
}}"#
    )
}

/// Parse LLM output into a ClarificationResult.
fn parse_response(content: &str) -> Result<ClarificationResult, ProviderError> {
    let trimmed = content.trim();

    // Strip markdown fences if present.
    let json_str = if trimmed.starts_with("```") {
        let start = trimmed
            .find('{')
            .ok_or_else(|| ProviderError::Other("no JSON in clarification response".into()))?;
        let end = trimmed.rfind('}').ok_or_else(|| {
            ProviderError::Other("no closing brace in clarification response".into())
        })?;
        &trimmed[start..=end]
    } else {
        trimmed
    };

    serde_json::from_str::<ClarificationResult>(json_str)
        .map_err(|e| ProviderError::Other(format!("failed to parse clarification JSON: {e}")))
}

/// Ensure every select question has an "Other (type your own)" option.
fn add_other_option(questions: &mut [Question]) {
    for q in questions.iter_mut() {
        if matches!(
            q.question_type,
            QuestionType::SingleSelect | QuestionType::MultiSelect
        ) && !q.options.iter().any(|o| o.starts_with("Other"))
        {
            q.options.push("Other (type your own)".into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json() {
        let json = r#"{
            "questions": [{
                "id": "q1", "question_type": "SingleSelect",
                "question": "Framework?",
                "options": ["React", "Vue"],
                "reason": "setup"
            }],
            "assumptions": ["TypeScript"],
            "confidence": 0.7,
            "sufficient_for_planning": false
        }"#;
        let result = parse_response(json).unwrap();
        assert_eq!(result.questions.len(), 1);
        assert_eq!(result.confidence, 0.7);
    }

    #[test]
    fn parse_with_code_fences() {
        let json = "```json\n{\"questions\":[],\"assumptions\":[],\"confidence\":0.9,\"sufficient_for_planning\":true}\n```";
        let result = parse_response(json).unwrap();
        assert!(result.questions.is_empty());
    }

    #[test]
    fn parse_invalid_errors() {
        assert!(parse_response("not json").is_err());
    }

    #[test]
    fn add_other_option_appends() {
        let mut questions = vec![Question {
            id: "q1".into(),
            question_type: QuestionType::SingleSelect,
            question: "Pick".into(),
            options: vec!["A".into(), "B".into()],
            reason: "test".into(),
        }];
        add_other_option(&mut questions);
        assert_eq!(
            questions[0].options.last().unwrap(),
            "Other (type your own)"
        );
    }

    #[test]
    fn add_other_option_skips_if_exists() {
        let mut questions = vec![Question {
            id: "q1".into(),
            question_type: QuestionType::SingleSelect,
            question: "Pick".into(),
            options: vec!["A".into(), "Other (please specify)".into()],
            reason: "test".into(),
        }];
        add_other_option(&mut questions);
        assert_eq!(questions[0].options.len(), 2); // Not duplicated.
    }

    #[test]
    fn system_prompt_greenfield() {
        let prompt = build_system_prompt(true);
        assert!(prompt.contains("GREENFIELD"));
        assert!(prompt.contains("Framework preferences"));
    }

    #[test]
    fn system_prompt_brownfield() {
        let prompt = build_system_prompt(false);
        assert!(prompt.contains("EXISTING"));
        assert!(prompt.contains("Scope of the change"));
    }
}
