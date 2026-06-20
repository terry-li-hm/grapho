use crate::models::{GraphoReport, OrphansReport};
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

    out.push_str(&format!(
        "budget: {} (remaining: {})\n",
        report.budget, remaining
    ));
    if !report.top_hits.is_empty() {
        out.push_str("Top hits:\n");
        for hit in &report.top_hits {
            out.push_str(&format!("  {}x  {}\n", hit.count, hit.key));
        }
    }
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
    if !report.top_hits.is_empty() {
        out.push_str("## Top hits\n");
        for hit in &report.top_hits {
            out.push_str(&format!("- {}x  {}\n", hit.count, hit.key));
        }
        out.push('\n');
    }
    out.push_str("## Sections\n");
    for section in &report.sections {
        out.push_str(&format!("- {}\n", section));
    }
    out
}

pub fn render_orphans(report: &OrphansReport, format: OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(report)?),
        OutputFormat::Human => {
            if io::stdout().is_terminal() {
                Ok(render_orphans_human_tty(report))
            } else {
                Ok(render_orphans_human_markdown(report))
            }
        }
    }
}

fn render_orphans_human_tty(report: &OrphansReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", "grapho orphans".bold()));
    out.push_str(&format!("memory: {}\n", report.memory_dir));
    out.push_str(&format!("marks: {}\n", report.marks_dir));
    out.push_str(&format!("min age (days): {}\n", report.min_age_days));

    let count_str = if report.count > 0 {
        report.count.to_string().red().to_string()
    } else {
        report.count.to_string().green().to_string()
    };
    out.push_str(&format!("orphans: {}\n", count_str));

    for o in &report.orphans {
        out.push_str(&format!("  {}d  {}\n", o.age_days, o.name));
    }
    out
}

fn render_orphans_human_markdown(report: &OrphansReport) -> String {
    let mut out = String::new();
    out.push_str("# grapho orphans\n\n");
    out.push_str(&format!("- memory: {}\n", report.memory_dir));
    out.push_str(&format!("- marks: {}\n", report.marks_dir));
    out.push_str(&format!("- min age (days): {}\n", report.min_age_days));
    out.push_str(&format!("- orphans: {}\n\n", report.count));

    for o in &report.orphans {
        out.push_str(&format!("- {}d  {}\n", o.age_days, o.name));
    }
    out
}
