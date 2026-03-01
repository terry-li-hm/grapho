use crate::models::GraphoReport;
use anyhow::Result;
use clap::ValueEnum;
use owo_colors::OwoColorize;
use std::io::{self, IsTerminal};

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

pub fn render(report: &GraphoReport, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(report)?),
        OutputFormat::Human => {
            if io::stdout().is_terminal() {
                Ok(render_human_tty(report))
            } else {
                Ok(render_human_markdown(report))
            }
        }
    }
}

fn render_human_tty(report: &GraphoReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", "grapho status".bold()));
    out.push_str(&format!("memory: {}\n", report.memory_path));
    out.push_str(&format!("line count: {}\n", report.line_count));

    let remaining = if report.over_budget {
        report.remaining.to_string().red().to_string()
    } else {
        report.remaining.to_string().green().to_string()
    };

    out.push_str(&format!("budget: {} (remaining: {})\n", report.budget, remaining));
    out.push_str("sections:\n");
    for section in &report.sections {
        out.push_str(&format!("- {}\n", section));
    }
    out
}

fn render_human_markdown(report: &GraphoReport) -> String {
    let mut out = String::new();
    out.push_str("# grapho status\n\n");
    out.push_str(&format!("- memory: {}\n", report.memory_path));
    out.push_str(&format!("- line count: {}\n", report.line_count));
    out.push_str(&format!(
        "- budget: {}\n- remaining: {}\n- over budget: {}\n\n",
        report.budget, report.remaining, report.over_budget
    ));
    out.push_str("## Sections\n");
    for section in &report.sections {
        out.push_str(&format!("- {}\n", section));
    }
    out
}
