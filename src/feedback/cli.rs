use std::io::{self, IsTerminal, Write};

use anyhow::Result;
use colored::Colorize;

use crate::feedback::{self, FeedbackSubmission, FeedbackSurface};
use crate::{api, idempotency, storage};

pub(crate) fn run_feedback_flow(rt: &tokio::runtime::Runtime) -> Result<()> {
    run_feedback_flow_seeded(rt, None, None, None)
}

pub(crate) fn run_feedback_flow_seeded(
    rt: &tokio::runtime::Runtime,
    seed_category: Option<feedback::FeedbackCategory>,
    seed_issue_code: Option<&str>,
    seed_message: Option<&str>,
) -> Result<()> {
    let schema = feedback::schema_for(FeedbackSurface::TerminalUi);

    println!();
    println!("{}", "Feedback".bright_cyan().bold());
    println!("  {}", schema.intro.bright_white());
    println!("  {}", schema.privacy_note.bright_black());
    println!();

    let category_index = match seed_category {
        Some(seed) => schema
            .categories
            .iter()
            .position(|c| c.id == seed)
            .unwrap_or(0),
        None => prompt_choice(
            "Pick a category:",
            &schema
                .categories
                .iter()
                .map(|c| format!("{} — {}", c.label, c.description))
                .collect::<Vec<_>>(),
        )?,
    };
    let category = schema.categories[category_index].id;

    let issues = &schema.categories[category_index].issues;
    let issue_index = match seed_issue_code {
        Some(code) => issues
            .iter()
            .position(|i| i.code.eq_ignore_ascii_case(code))
            .unwrap_or(0),
        None => prompt_choice(
            "Pick what best matches your issue:",
            &issues
                .iter()
                .map(|i| format!("{} — {}", i.label, i.hint))
                .collect::<Vec<_>>(),
        )?,
    };
    let issue = &issues[issue_index];

    println!();
    println!(
        "{}",
        "Describe what happened (press Enter on an empty line to finish):".bright_white()
    );
    if let Some(seed) = seed_message {
        if !seed.trim().is_empty() {
            println!("{}", "Suggested starter (edit as needed):".bright_black());
            println!("{}\n", seed.bright_black());
        }
    }

    let mut message = read_multiline_message()?;
    if message.trim().is_empty() {
        // If the user didn't type anything, fall back to the seeded message (if any).
        if let Some(seed) = seed_message {
            message = seed.trim().to_string();
        }
    }
    if message.trim().is_empty() {
        println!("{}", "Cancelled (no message entered).".bright_yellow());
        return Ok(());
    }

    let include_diagnostics = prompt_yes_no(
        "Include an optional diagnostics summary (recommended for bugs)? [y/N]: ",
        false,
    )?;

    let diagnostics =
        include_diagnostics.then(|| feedback::collect_diagnostics(FeedbackSurface::TerminalUi));
    let submission = FeedbackSubmission {
        surface: FeedbackSurface::TerminalUi,
        category,
        issue_code: issue.code.to_string(),
        message,
        diagnostics,
    };

    if let Err(errors) = submission.validate() {
        println!("{}", "Validation issues:".bright_red());
        for e in errors {
            println!("  - {}", e.bright_red());
        }
        return Ok(());
    }

    println!();
    println!("{}", "Submitting feedback...".bright_cyan());

    let idempotency_key = idempotency::new_feedback_key();
    match rt.block_on(api::submit_feedback_with_idempotency_key(
        &submission,
        &idempotency_key,
    )) {
        Ok(_response) => {
            println!("{}", "✓ Feedback sent. Thank you.".bright_green());
            Ok(())
        }
        Err(err) => {
            if api::should_queue_offline_feedback(&err) {
                let storage = storage::init_storage()?;
                let _pending_id = storage
                    .save_pending_feedback_with_idempotency_key(&submission, &idempotency_key)?;
                println!(
                    "{}",
                    "⚠ Could not send right now; saved locally for automatic retry."
                        .bright_yellow()
                );
                println!(
                    "{} {}",
                    "Reason:".bright_yellow(),
                    err.to_string().bright_red()
                );
                Ok(())
            } else {
                Err(anyhow::anyhow!(err.to_string()))
            }
        }
    }
}

pub(crate) fn offer_feedback_prompt(
    rt: &tokio::runtime::Runtime,
    seed_category: feedback::FeedbackCategory,
    seed_issue_code: &str,
    seed_message: &str,
) -> Result<()> {
    if !io::stdout().is_terminal() {
        println!(
            "{} {}",
            "Tip:".bright_black(),
            "You can send feedback with: fps-tracker feedback".bright_black()
        );
        return Ok(());
    }

    println!();
    let open_now = prompt_yes_no(
        "Would you like to send feedback to help us fix this? [y/N]: ",
        false,
    )?;
    if !open_now {
        println!(
            "{} {}",
            "No problem.".bright_black(),
            "You can run `fps-tracker feedback` anytime.".bright_black()
        );
        return Ok(());
    }

    run_feedback_flow_seeded(
        rt,
        Some(seed_category),
        Some(seed_issue_code),
        Some(seed_message),
    )
}

fn prompt_choice(prompt: &str, options: &[String]) -> Result<usize> {
    loop {
        println!("{}", prompt.bright_white());
        for (i, opt) in options.iter().enumerate() {
            println!("  {} {}", format!("{:>2}.", i + 1).bright_black(), opt);
        }
        print!("{} ", "Enter a number:".bright_yellow());
        let _ = io::stdout().flush();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            println!("{}", "Please enter a number.".bright_yellow());
            continue;
        }
        let parsed = trimmed.parse::<usize>().ok();
        if let Some(n) = parsed {
            if n >= 1 && n <= options.len() {
                return Ok(n - 1);
            }
        }
        println!("{}", "Invalid choice. Try again.".bright_red());
        println!();
    }
}

fn prompt_yes_no(prompt: &str, default_yes: bool) -> Result<bool> {
    loop {
        print!("{prompt}");
        let _ = io::stdout().flush();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Ok(default_yes);
        }
        match trimmed.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                println!("{}", "Please answer y or n.".bright_yellow());
            }
        }
    }
}

fn read_multiline_message() -> Result<String> {
    let mut lines = Vec::new();
    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let line = input.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            break;
        }
        lines.push(line.to_string());
    }
    Ok(lines.join("\n"))
}
