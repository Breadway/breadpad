use anyhow::{Context, Result};
use breadpad_shared::{
    classifier::Classifier,
    config::OllamaConfig,
    parser::parse_rule_based,
    types::ClassificationResult,
};
use chrono::{DateTime, Local, Timelike, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use comfy_table::{Cell, Color, Table};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const DEFAULT_CORPUS: &str = "breadpad-test/corpus.json";
const DEFAULT_MORNING: &str = "08:00";

#[derive(Parser)]
#[command(
    name = "breadpad-test",
    about = "Test harness for the breadpad classification pipeline"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run tests against the corpus
    Run {
        #[arg(long, default_value = DEFAULT_CORPUS)]
        corpus: PathBuf,
        /// Which classification tiers to invoke: 1 (rule-based only), 2 (+ ONNX), 3/all (+ Ollama)
        #[arg(long, default_value = "1")]
        tier: TierArg,
        /// Output format: table, json, or failures (failing cases only)
        #[arg(long, default_value = "table")]
        format: FormatArg,
    },
    /// Interactively add a new corpus entry
    Add {
        #[arg(long, default_value = DEFAULT_CORPUS)]
        corpus: PathBuf,
    },
    /// Show a corpus entry and the pipeline's actual output side by side
    Show {
        index: usize,
        #[arg(long, default_value = DEFAULT_CORPUS)]
        corpus: PathBuf,
    },
    /// Open the corpus file in $EDITOR at the given entry
    Edit {
        index: usize,
        #[arg(long, default_value = DEFAULT_CORPUS)]
        corpus: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum TierArg {
    #[value(name = "1")]
    One,
    #[value(name = "2")]
    Two,
    #[value(name = "3")]
    Three,
    #[value(name = "all")]
    All,
}

impl TierArg {
    fn label(&self) -> &'static str {
        match self {
            TierArg::One => "1",
            TierArg::Two => "2",
            TierArg::Three => "3",
            TierArg::All => "all",
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
enum FormatArg {
    Table,
    Json,
    Failures,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CorpusEntry {
    input: String,
    #[serde(default)]
    expected_type: Option<String>,
    /// Expected time as HH:MM — date component is ignored so tests are not date-sensitive
    #[serde(default)]
    expected_time: Option<String>,
    /// Expected body text (checked as substring of actual body)
    #[serde(default)]
    expected_body: Option<String>,
    /// Expected rrule (checked as substring of actual rrule string)
    #[serde(default)]
    expected_rrule: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
struct TestResult {
    index: usize,
    input: String,
    tier_used: String,
    actual_type: String,
    actual_time: Option<String>,
    actual_rrule: Option<String>,
    actual_body: String,
    type_pass: Option<bool>,
    time_pass: Option<bool>,
    rrule_pass: Option<bool>,
    body_pass: Option<bool>,
    pass: bool,
    failure_reason: Option<String>,
}

fn load_corpus(path: &Path) -> Result<Vec<CorpusEntry>> {
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("could not read corpus at {}", path.display()))?;
    serde_json::from_str(&data).context("invalid corpus JSON")
}

fn save_corpus(path: &Path, entries: &[CorpusEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(entries)?)?;
    Ok(())
}

fn classify_with_tier(text: &str, tier: &TierArg) -> ClassificationResult {
    match tier {
        TierArg::One => parse_rule_based(text, DEFAULT_MORNING),
        TierArg::Two => {
            let mut clf = Classifier::load("auto", DEFAULT_MORNING);
            clf.classify(text)
        }
        TierArg::Three | TierArg::All => {
            let ollama = OllamaConfig {
                endpoint: "http://localhost:11434".to_string(),
                model: "llama3.2:3b".to_string(),
                confidence_threshold: 0.6,
                enabled: true,
            };
            let mut clf = Classifier::load("auto", DEFAULT_MORNING).with_ollama(ollama);
            clf.classify(text)
        }
    }
}

fn format_time(t: Option<DateTime<Utc>>) -> Option<String> {
    t.map(|dt| {
        let local: DateTime<Local> = dt.into();
        format!("{:02}:{:02}", local.hour(), local.minute())
    })
}

fn run_tests(entries: &[CorpusEntry], tier: &TierArg) -> Vec<TestResult> {
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let result = classify_with_tier(&entry.input, tier);
            let actual_type = result.note_type.as_str().to_string();
            let actual_time = format_time(result.time);
            let actual_rrule = result.rrule.as_ref().map(|r| r.as_str().to_string());
            let actual_body = result.body.clone();

            let type_pass = entry
                .expected_type
                .as_ref()
                .map(|et| et.to_lowercase() == actual_type);

            let time_pass = entry.expected_time.as_ref().map(|et| {
                actual_time.as_deref().map(|at| at == et).unwrap_or(false)
            });

            let rrule_pass = entry.expected_rrule.as_ref().map(|er| {
                actual_rrule
                    .as_deref()
                    .map(|ar| ar.contains(er.as_str()))
                    .unwrap_or(false)
            });

            let body_pass = entry.expected_body.as_ref().map(|eb| {
                actual_body
                    .to_lowercase()
                    .contains(&eb.to_lowercase())
            });

            let mut failure_parts: Vec<String> = Vec::new();
            if type_pass == Some(false) {
                failure_parts.push(format!(
                    "type: expected {}, got {}",
                    entry.expected_type.as_deref().unwrap_or("?"),
                    actual_type
                ));
            }
            if time_pass == Some(false) {
                failure_parts.push(format!(
                    "time: expected {}, got {}",
                    entry.expected_time.as_deref().unwrap_or("none"),
                    actual_time.as_deref().unwrap_or("none")
                ));
            }
            if rrule_pass == Some(false) {
                failure_parts.push(format!(
                    "rrule: expected to contain {:?}",
                    entry.expected_rrule.as_deref().unwrap_or("")
                ));
            }
            if body_pass == Some(false) {
                failure_parts.push(format!(
                    "body: {:?} not found in {:?}",
                    entry.expected_body.as_deref().unwrap_or(""),
                    actual_body
                ));
            }

            let pass = failure_parts.is_empty();

            TestResult {
                index: i + 1,
                input: entry.input.clone(),
                tier_used: tier.label().to_string(),
                actual_type,
                actual_time,
                actual_rrule,
                actual_body,
                type_pass,
                time_pass,
                rrule_pass,
                body_pass,
                pass,
                failure_reason: if failure_parts.is_empty() {
                    None
                } else {
                    Some(failure_parts.join("; "))
                },
            }
        })
        .collect()
}

fn check_mark(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "✓",
        Some(false) => "✗",
        None => "-",
    }
}

fn print_table(results: &[TestResult], failures_only: bool) {
    let to_show: Vec<&TestResult> = if failures_only {
        results.iter().filter(|r| !r.pass).collect()
    } else {
        results.iter().collect()
    };

    let mut table = Table::new();
    table.set_header(vec!["#", "input", "tier", "type", "time", "rrule", "body", "result"]);

    for r in &to_show {
        let input_display = if r.input.len() > 42 {
            format!("{}…", &r.input[..41])
        } else {
            r.input.clone()
        };

        let result_cell = if r.pass {
            Cell::new("PASS").fg(Color::Green)
        } else {
            let reason = r.failure_reason.as_deref().unwrap_or("");
            Cell::new(format!("FAIL  {reason}")).fg(Color::Red)
        };

        table.add_row(vec![
            Cell::new(r.index),
            Cell::new(&input_display),
            Cell::new(&r.tier_used),
            Cell::new(&r.actual_type),
            Cell::new(check_mark(r.time_pass)),
            Cell::new(check_mark(r.rrule_pass)),
            Cell::new(check_mark(r.body_pass)),
            result_cell,
        ]);
    }

    if !to_show.is_empty() {
        println!("{table}");
    }

    let passed = results.iter().filter(|r| r.pass).count();
    let failed = results.iter().filter(|r| !r.pass).count();
    if failed == 0 {
        println!("{}", format!("{passed} passed").green());
    } else {
        println!("{}, {}", format!("{passed} passed").green(), format!("{failed} failed").red());
    }
}

fn prompt(label: &str) -> Result<String> {
    use std::io::Write;
    print!("{label}");
    std::io::stdout().flush()?;
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

fn prompt_opt(label: &str) -> Result<Option<String>> {
    let s = prompt(label)?;
    Ok(if s.is_empty() { None } else { Some(s) })
}

fn cmd_add(corpus_path: &Path) -> Result<()> {
    let mut entries = if corpus_path.exists() {
        load_corpus(corpus_path)?
    } else {
        vec![]
    };

    println!("Adding a new corpus entry. Press Enter to leave a field null.\n");

    let input = prompt("input: ")?;
    if input.is_empty() {
        anyhow::bail!("input is required");
    }
    let expected_type = prompt_opt("expected_type (todo/reminder/idea/note/question or Enter): ")?;
    let expected_time = prompt_opt("expected_time (HH:MM or Enter): ")?;
    let expected_body = prompt_opt("expected_body (substring or Enter): ")?;
    let expected_rrule = prompt_opt("expected_rrule (substring or Enter): ")?;
    let notes = prompt_opt("notes (or Enter): ")?;

    entries.push(CorpusEntry {
        input,
        expected_type,
        expected_time,
        expected_body,
        expected_rrule,
        notes,
    });

    save_corpus(corpus_path, &entries)?;
    println!("\nAdded entry #{}", entries.len());
    Ok(())
}

fn cmd_show(index: usize, corpus_path: &Path, tier: &TierArg) -> Result<()> {
    let entries = load_corpus(corpus_path)?;
    let entry = entries.get(index.saturating_sub(1)).ok_or_else(|| {
        anyhow::anyhow!(
            "index {} out of range (corpus has {} entries)",
            index,
            entries.len()
        )
    })?;

    let result = classify_with_tier(&entry.input, tier);
    let actual_time = format_time(result.time);
    let actual_rrule = result.rrule.as_ref().map(|r| r.as_str().to_string());

    println!("─── Entry {} (tier {}) ───", index, tier.label());
    println!("input:  {}", entry.input);
    if let Some(n) = &entry.notes {
        println!("notes:  {}", n);
    }
    println!();

    let sep = "─".repeat(62);
    println!("{:<14} {:<26} {}", "field", "expected", "actual");
    println!("{sep}");
    println!(
        "{:<14} {:<26} {}",
        "type",
        entry.expected_type.as_deref().unwrap_or("(any)"),
        result.note_type.as_str()
    );
    println!(
        "{:<14} {:<26} {}",
        "time",
        entry.expected_time.as_deref().unwrap_or("(any)"),
        actual_time.as_deref().unwrap_or("none")
    );
    println!(
        "{:<14} {:<26} {}",
        "rrule",
        entry.expected_rrule.as_deref().unwrap_or("(any)"),
        actual_rrule.as_deref().unwrap_or("none")
    );
    println!(
        "{:<14} {:<26} {}",
        "body",
        entry.expected_body.as_deref().unwrap_or("(any)"),
        result.body
    );
    println!("{:<14} {:<26} {:.2}", "confidence", "", result.confidence);

    Ok(())
}

fn find_entry_line(corpus_path: &Path, index: usize) -> Result<Option<usize>> {
    let content = std::fs::read_to_string(corpus_path)?;
    let mut count = 0usize;
    for (line_num, line) in content.lines().enumerate() {
        if line.trim_start().starts_with('{') {
            count += 1;
            if count == index {
                return Ok(Some(line_num + 1));
            }
        }
    }
    Ok(None)
}

fn cmd_edit(index: usize, corpus_path: &Path) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let line = find_entry_line(corpus_path, index)?;

    let status = if let Some(n) = line {
        std::process::Command::new(&editor)
            .arg(format!("+{n}"))
            .arg(corpus_path)
            .status()?
    } else {
        std::process::Command::new(&editor)
            .arg(corpus_path)
            .status()?
    };

    if !status.success() {
        anyhow::bail!("editor exited with non-zero status");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { corpus, tier, format } => {
            let entries = load_corpus(&corpus)?;
            if entries.is_empty() {
                println!("corpus is empty");
                return Ok(());
            }
            let results = run_tests(&entries, &tier);
            match format {
                FormatArg::Table => print_table(&results, false),
                FormatArg::Failures => print_table(&results, true),
                FormatArg::Json => println!("{}", serde_json::to_string_pretty(&results)?),
            }
            let failed = results.iter().filter(|r| !r.pass).count();
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Commands::Add { corpus } => cmd_add(&corpus)?,
        Commands::Show { index, corpus } => cmd_show(index, &corpus, &TierArg::One)?,
        Commands::Edit { index, corpus } => cmd_edit(index, &corpus)?,
    }

    Ok(())
}
