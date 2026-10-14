use std::process::ExitCode;

use clap::{Parser, Subcommand};
use girth::config::{Context, resolve_roots};
use girth::measure::{format_table, measure};
use girth::skill::{SkillInfo, discover_skills};

/// Just-in-time, project-scoped agent skills.
#[derive(Parser)]
#[command(name = "girth", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Report what installed skills cost in context.
    Measure {
        /// Skill root dirs (default: GIRTH_SKILL_ROOTS, .girth/config.json, standard locations).
        #[arg(long, num_args = 1..)]
        roots: Vec<String>,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
}

fn warn(warnings: &[String]) {
    for w in warnings {
        eprintln!("{w}");
    }
}

fn report_fallbacks(skills: &[SkillInfo]) {
    let names: Vec<&str> = skills
        .iter()
        .filter(|s| s.fell_back)
        .map(|s| s.name.as_str())
        .collect();
    if !names.is_empty() {
        eprintln!(
            "girth: recovered invalid YAML frontmatter leniently in {} skill(s): {}",
            names.len(),
            names.join(", ")
        );
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = Context::from_process();
    let mut warnings = Vec::new();
    match cli.command {
        Command::Measure { roots, json } => {
            let roots = resolve_roots(&roots, &ctx, &mut warnings);
            warn(&warnings);
            let skills = discover_skills(&roots);
            report_fallbacks(&skills);
            let report = measure(&skills);
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serialises")
                );
            } else {
                println!("{}", format_table(&report));
            }
        }
    }
    ExitCode::SUCCESS
}
