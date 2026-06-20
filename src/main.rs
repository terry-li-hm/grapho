mod hits;
mod models;
mod orphans;
mod output;
mod parse;
mod paths;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use clap::{Parser, Subcommand};
use hits::{HitStore, entry_key, increment, load_hits, save_hits};
use models::{GraphoReport, HitEntry, MemoryEntry, Orphan, OrphansReport, Section};
use orphans::{find_orphan_candidates, older_than};
use output::OutputFormat;
use parse::{MemoryDoc, read_doc, write_doc_atomic, write_string_atomic};
use paths::{
    resolve_hits_path, resolve_marks_dir, resolve_memory_dir, resolve_memory_path,
    resolve_overflow_path, resolve_solutions_dir,
};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

const BUDGET: usize = 150;

#[derive(Debug)]
struct OverBudgetError;

impl std::fmt::Display for OverBudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("memory budget exceeded")
    }
}

impl std::error::Error for OverBudgetError {}

#[derive(Parser)]
#[command(name = "grapho", version, about = "Manage personal memory files")]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show MEMORY.md status and budget usage
    Status,
    /// Add a new MEMORY.md entry interactively
    Add,
    /// Move an entry from MEMORY.md to memory-overflow.md
    Demote { search: String },
    /// Move an entry from memory-overflow.md to MEMORY.md
    Promote { search: String },
    /// Record a hit for a MEMORY.md entry
    Hit { search: String },
    /// Review overflow entries one by one
    Review,
    /// Scaffold a solution note under ~/docs/solutions
    Solution { name: String },
    /// List CC-auto-memory findings with no authoritative epigenome/marks counterpart
    Orphans {
        /// Only list findings older than this many days (by mtime)
        #[arg(long, default_value_t = 7)]
        min_age_days: i64,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if err.downcast_ref::<OverBudgetError>().is_some() {
                return ExitCode::from(1);
            }
            eprintln!("Fatal error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Status => cmd_status(cli.format),
        Command::Add => cmd_add(),
        Command::Demote { search } => cmd_demote(&search),
        Command::Promote { search } => cmd_promote(&search),
        Command::Hit { search } => cmd_hit(&search),
        Command::Review => cmd_review(),
        Command::Solution { name } => cmd_solution(&name),
        Command::Orphans { min_age_days } => cmd_orphans(cli.format, min_age_days),
    }
}

fn cmd_status(format: OutputFormat) -> Result<()> {
    let memory_path = resolve_memory_path()?;
    let hits_path = resolve_hits_path()?;
    let content = fs::read_to_string(&memory_path)
        .with_context(|| format!("failed to read {}", memory_path.display()))?;
    let doc = parse::parse_doc(&content)?;
    let line_count = content.lines().count();
    let remaining = BUDGET as i64 - line_count as i64;
    let hit_store = load_hits(&hits_path)?;
    let top_hits = top_hits(&hit_store);

    let report = GraphoReport {
        memory_path: memory_path.display().to_string(),
        line_count,
        budget: BUDGET,
        remaining,
        over_budget: remaining < 0,
        sections: doc.sections.iter().map(|s| s.name.clone()).collect(),
        top_hits,
    };

    println!("{}", output::render(&report, format)?);

    if report.over_budget {
        return Err(anyhow!(OverBudgetError));
    }

    Ok(())
}

fn cmd_add() -> Result<()> {
    if !is_interactive() {
        bail!("add requires interactive terminal");
    }

    let memory_path = resolve_memory_path()?;
    let mut doc = read_doc(&memory_path)?;

    let selected_section = if doc.sections.is_empty() {
        let name = prompt("Section name: ")?;
        if name.trim().is_empty() {
            bail!("section name cannot be empty");
        }
        doc.sections.push(Section {
            name: name.trim().to_string(),
            entries: Vec::new(),
        });
        0
    } else {
        let mut options: Vec<String> = doc.sections.iter().map(|s| s.name.clone()).collect();
        options.push("New section".to_string());
        let selected = choose_option("Choose a section", &options)?;

        if selected == options.len() - 1 {
            let name = prompt("New section name: ")?;
            if name.trim().is_empty() {
                bail!("section name cannot be empty");
            }
            doc.sections.push(Section {
                name: name.trim().to_string(),
                entries: Vec::new(),
            });
            doc.sections.len() - 1
        } else {
            selected
        }
    };

    let entry = prompt("Entry text: ")?;
    let entry = entry.trim();
    if entry.is_empty() {
        bail!("entry text cannot be empty");
    }
    let entry = normalize_entry(entry);

    doc.sections[selected_section].entries.push(entry.clone());
    write_doc_atomic(&memory_path, &doc)?;

    println!(
        "Added entry to {}: {}",
        doc.sections[selected_section].name, entry
    );
    Ok(())
}

fn cmd_demote(search: &str) -> Result<()> {
    let memory_path = resolve_memory_path()?;
    let overflow_path = resolve_overflow_path()?;
    let hits_path = resolve_hits_path()?;

    let mut memory_doc = read_doc(&memory_path)?;
    let mut overflow_doc = read_or_init_overflow_doc(&overflow_path)?;
    let hit_store = load_hits(&hits_path)?;

    let matches = find_matches(&memory_doc, search);
    if matches.is_empty() {
        bail!("no matches found for '{search}'");
    }

    let choice = select_match_with_hits("MEMORY.md matches", &matches, &hit_store)?;
    let selected = &matches[choice];

    let moved_entry = memory_doc.sections[selected.section_idx]
        .entries
        .remove(selected.entry_idx);

    let overflow_section_idx = ensure_section(&mut overflow_doc, &selected.entry.section);
    overflow_doc.sections[overflow_section_idx]
        .entries
        .push(moved_entry.clone());

    write_doc_atomic(&overflow_path, &overflow_doc)?;
    write_doc_atomic(&memory_path, &memory_doc)?;

    println!(
        "Demoted from {} to {} under [{}]: {}",
        memory_path.display(),
        overflow_path.display(),
        selected.entry.section,
        moved_entry
    );

    Ok(())
}

fn cmd_hit(search: &str) -> Result<()> {
    let memory_path = resolve_memory_path()?;
    let hits_path = resolve_hits_path()?;
    let memory_doc = read_doc(&memory_path)?;

    let matches = find_matches(&memory_doc, search);
    if matches.is_empty() {
        bail!("no matches found for '{search}'");
    }

    let choice = if matches.len() == 1 {
        0
    } else if is_interactive() {
        select_match("MEMORY.md matches", &matches)?
    } else {
        bail!("multiple matches; rerun interactively");
    };

    let selected = &matches[choice];
    let key = entry_key(&selected.entry.text);
    let mut store = load_hits(&hits_path)?;
    increment(&mut store, &key);
    let count = store.get(&key).map(|record| record.count).unwrap_or(0);
    save_hits(&hits_path, &store)?;

    println!(
        "Hit recorded [{count}]: {}",
        entry_preview(&selected.entry.text)
    );
    Ok(())
}

fn cmd_promote(search: &str) -> Result<()> {
    if !is_interactive() {
        bail!("promote requires interactive terminal");
    }

    let memory_path = resolve_memory_path()?;
    let overflow_path = resolve_overflow_path()?;

    let mut memory_doc = read_doc(&memory_path)?;
    let mut overflow_doc = read_doc(&overflow_path)
        .with_context(|| format!("failed to parse {}", overflow_path.display()))?;

    let matches = find_matches(&overflow_doc, search);
    if matches.is_empty() {
        bail!("no matches found for '{search}'");
    }

    let choice = select_match("Overflow matches", &matches)?;
    let selected = &matches[choice];

    let target_section_idx = select_target_section(&mut memory_doc)?;
    let moved_entry = overflow_doc.sections[selected.section_idx]
        .entries
        .remove(selected.entry_idx);

    memory_doc.sections[target_section_idx]
        .entries
        .push(moved_entry.clone());

    write_doc_atomic(&memory_path, &memory_doc)?;
    write_doc_atomic(&overflow_path, &overflow_doc)?;

    println!(
        "Promoted from {} to {} under [{}]: {}",
        overflow_path.display(),
        memory_path.display(),
        memory_doc.sections[target_section_idx].name,
        moved_entry
    );

    Ok(())
}

fn cmd_review() -> Result<()> {
    if !is_interactive() {
        bail!("review requires interactive terminal");
    }

    let memory_path = resolve_memory_path()?;
    let overflow_path = resolve_overflow_path()?;

    let mut memory_doc = read_doc(&memory_path)?;
    let mut overflow_doc = read_doc(&overflow_path)
        .with_context(|| format!("failed to parse {}", overflow_path.display()))?;

    let entries = all_entries(&overflow_doc);
    if entries.is_empty() {
        println!("No overflow entries to review.");
        return Ok(());
    }

    let age_days = overflow_age_days(&overflow_path)?;

    let mut promoted = 0usize;
    let mut deleted = 0usize;
    let mut kept = 0usize;

    for entry in entries {
        println!();
        println!(
            "[{}] {} (age: {} days)",
            entry.section, entry.text, age_days
        );

        loop {
            let action = prompt("Action: [p]romote / [k]eep / [d]elete: ")?;
            match action.trim().to_ascii_lowercase().as_str() {
                "p" => {
                    let target_section_idx = select_target_section(&mut memory_doc)?;
                    remove_entry(&mut overflow_doc, &entry)?;
                    memory_doc.sections[target_section_idx]
                        .entries
                        .push(entry.text.clone());
                    promoted += 1;
                    break;
                }
                "k" => {
                    kept += 1;
                    break;
                }
                "d" => {
                    remove_entry(&mut overflow_doc, &entry)?;
                    deleted += 1;
                    break;
                }
                _ => {
                    println!("Invalid choice. Use p, k, or d.");
                }
            }
        }
    }

    if promoted > 0 {
        write_doc_atomic(&memory_path, &memory_doc)?;
    }
    if promoted > 0 || deleted > 0 {
        write_doc_atomic(&overflow_path, &overflow_doc)?;
    }

    println!(
        "Review complete: {} promoted, {} deleted, {} kept",
        promoted, deleted, kept
    );

    Ok(())
}

fn cmd_solution(name: &str) -> Result<()> {
    let solutions_dir = resolve_solutions_dir()?;
    fs::create_dir_all(&solutions_dir)
        .with_context(|| format!("failed to create {}", solutions_dir.display()))?;

    let path = solutions_dir.join(format!("{name}.md"));
    if path.exists() {
        println!("{}", path.display());
        return Ok(());
    }

    let body = format!("# {name}\n\n## Problem\n\n## Solution\n\n## Gotchas\n\n## References\n");

    write_string_atomic(&path, &body)?;
    println!("{}", path.display());
    Ok(())
}

fn cmd_orphans(format: OutputFormat, min_age_days: i64) -> Result<()> {
    let memory_dir = resolve_memory_dir()?;
    let marks_dir = resolve_marks_dir()?;
    let candidates = find_orphan_candidates(&memory_dir, &marks_dir)?;
    let now = Utc::now().timestamp();

    let mut orphans = Vec::new();
    for cand in candidates {
        let modified = fs::metadata(&cand.path)
            .and_then(|m| m.modified())
            .with_context(|| format!("failed to read mtime for {}", cand.path.display()))?;
        let mtime_unix = chrono::DateTime::<Utc>::from(modified).timestamp();
        if !older_than(mtime_unix, now, min_age_days) {
            continue;
        }
        orphans.push(Orphan {
            name: cand.name,
            path: cand.path.display().to_string(),
            age_days: (now - mtime_unix) / 86_400,
            reason: "no epigenome counterpart (basename or content)".to_string(),
        });
    }
    orphans.sort_by(|a, b| {
        b.age_days
            .cmp(&a.age_days)
            .then_with(|| a.name.cmp(&b.name))
    });

    let report = OrphansReport {
        memory_dir: memory_dir.display().to_string(),
        marks_dir: marks_dir.display().to_string(),
        min_age_days,
        count: orphans.len(),
        orphans,
    };

    println!("{}", output::render_orphans(&report, format)?);
    Ok(())
}

fn is_interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn prompt(prompt_text: &str) -> Result<String> {
    print!("{prompt_text}");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .context("failed to read from stdin")?;

    Ok(buf.trim_end_matches(['\n', '\r']).to_string())
}

fn choose_option(title: &str, options: &[String]) -> Result<usize> {
    println!("{title}:");
    for (idx, option) in options.iter().enumerate() {
        println!("{}. {}", idx + 1, option);
    }

    loop {
        let raw = prompt("Choose number: ")?;
        let parsed = raw
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| *n >= 1 && *n <= options.len());

        if let Some(number) = parsed {
            return Ok(number - 1);
        }

        println!(
            "Invalid selection. Enter a number between 1 and {}.",
            options.len()
        );
    }
}

fn normalize_entry(entry: &str) -> String {
    if entry.starts_with("- ") {
        entry.to_string()
    } else {
        format!("- {entry}")
    }
}

fn select_target_section(memory_doc: &mut MemoryDoc) -> Result<usize> {
    if memory_doc.sections.is_empty() {
        let name = prompt("No sections found. New section name: ")?;
        if name.trim().is_empty() {
            bail!("section name cannot be empty");
        }

        memory_doc.sections.push(Section {
            name: name.trim().to_string(),
            entries: Vec::new(),
        });

        return Ok(0);
    }

    let options: Vec<String> = memory_doc.sections.iter().map(|s| s.name.clone()).collect();
    choose_option("Choose target section", &options)
}

fn read_or_init_overflow_doc(path: &Path) -> Result<MemoryDoc> {
    if path.exists() {
        return read_doc(path);
    }

    Ok(MemoryDoc {
        header_lines: vec![
            "# MEMORY.md Overflow".to_string(),
            String::new(),
            "Entries demoted from MEMORY.md to stay within 150-line budget.".to_string(),
        ],
        sections: Vec::new(),
        trailing_newline: true,
    })
}

fn ensure_section(doc: &mut MemoryDoc, name: &str) -> usize {
    if let Some(index) = doc.sections.iter().position(|s| s.name == name) {
        return index;
    }

    doc.sections.push(Section {
        name: name.to_string(),
        entries: Vec::new(),
    });
    doc.sections.len() - 1
}

#[derive(Clone, Debug)]
struct EntryMatch {
    section_idx: usize,
    entry_idx: usize,
    entry: MemoryEntry,
}

fn find_matches(doc: &MemoryDoc, search: &str) -> Vec<EntryMatch> {
    let needle = search.to_ascii_lowercase();
    let mut matches = Vec::new();

    for (section_idx, section) in doc.sections.iter().enumerate() {
        for (entry_idx, line) in section.entries.iter().enumerate() {
            if line.to_ascii_lowercase().contains(&needle) {
                matches.push(EntryMatch {
                    section_idx,
                    entry_idx,
                    entry: MemoryEntry {
                        section: section.name.clone(),
                        text: line.clone(),
                    },
                });
            }
        }
    }

    matches
}

fn select_match(title: &str, matches: &[EntryMatch]) -> Result<usize> {
    if matches.len() == 1 {
        return Ok(0);
    }

    if !is_interactive() {
        bail!("multiple matches found; rerun in an interactive terminal");
    }

    let options: Vec<String> = matches
        .iter()
        .map(|m| format!("[{}] {}", m.entry.section, m.entry.text))
        .collect();
    choose_option(title, &options)
}

fn select_match_with_hits(
    title: &str,
    matches: &[EntryMatch],
    hit_store: &HitStore,
) -> Result<usize> {
    if matches.len() == 1 {
        return Ok(0);
    }

    if !is_interactive() {
        bail!("multiple matches found; rerun in an interactive terminal");
    }

    let options: Vec<String> = matches
        .iter()
        .map(|m| {
            let key = entry_key(&m.entry.text);
            let count = hit_store.get(&key).map(|record| record.count).unwrap_or(0);
            format!("[{count} hits] [{}] {}", m.entry.section, m.entry.text)
        })
        .collect();
    choose_option(title, &options)
}

fn top_hits(store: &HitStore) -> Vec<HitEntry> {
    let mut entries: Vec<HitEntry> = store
        .iter()
        .map(|(key, record)| HitEntry {
            key: key.clone(),
            count: record.count,
            last: record.last.clone(),
        })
        .collect();
    entries.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    entries.truncate(5);
    entries
}

fn entry_preview(text: &str) -> String {
    text.chars().take(60).collect()
}

fn all_entries(doc: &MemoryDoc) -> Vec<MemoryEntry> {
    let mut entries = Vec::new();
    for section in &doc.sections {
        for entry in &section.entries {
            entries.push(MemoryEntry {
                section: section.name.clone(),
                text: entry.clone(),
            });
        }
    }
    entries
}

fn remove_entry(doc: &mut MemoryDoc, entry: &MemoryEntry) -> Result<()> {
    let section = doc
        .sections
        .iter_mut()
        .find(|section| section.name == entry.section)
        .ok_or_else(|| anyhow!("section '{}' not found while removing entry", entry.section))?;

    let pos = section
        .entries
        .iter()
        .position(|existing| existing == &entry.text)
        .ok_or_else(|| anyhow!("entry '{}' not found while removing", entry.text))?;

    section.entries.remove(pos);
    Ok(())
}

fn overflow_age_days(path: &Path) -> Result<i64> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for {}", path.display()))?;
    let modified = metadata
        .modified()
        .with_context(|| format!("failed to read modified time for {}", path.display()))?;

    let modified_utc: chrono::DateTime<Utc> = modified.into();
    let now = Utc::now();
    let days = (now - modified_utc).num_days();

    Ok(days.max(0))
}
