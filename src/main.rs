use std::process::ExitCode;

use clap::{Parser, Subcommand};
use girth::config::{Context, resolve_output_dir, resolve_roots};
use girth::measure::{format_table, measure, thousands};
use girth::search::{HitJson, rank};
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
    /// Rank local skills (and the crafted-skill cache) for a need.
    Search {
        /// The need, in words.
        query: String,
        /// Skill root dirs.
        #[arg(long, num_args = 1..)]
        roots: Vec<String>,
        /// Crafted-skill dir (the cache); always searched.
        #[arg(long)]
        output: Option<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
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
        Command::Search {
            query,
            roots,
            output,
            limit,
            json,
        } => {
            let output_dir = resolve_output_dir(output.as_deref(), &ctx, &mut warnings);
            let mut roots = resolve_roots(&roots, &ctx, &mut warnings);
            if !roots.contains(&output_dir) {
                roots.push(output_dir.clone());
            }
            warn(&warnings);
            let skills = discover_skills(&roots);
            report_fallbacks(&skills);
            let hits = rank(&skills, &query, limit);
            if json {
                let payload = serde_json::json!({
                    "output_dir": output_dir.display().to_string(),
                    "hits": hits.iter().map(HitJson::from).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("hits serialise")
                );
            } else {
                println!("output_dir: {}", output_dir.display());
                if hits.is_empty() {
                    println!("no matching skills");
                }
                for hit in &hits {
                    let flags = format!(
                        "{}{}",
                        if hit.skill.crafted() {
                            " [crafted]"
                        } else {
                            ""
                        },
                        if hit.exact { " [exact]" } else { "" }
                    );
                    println!(
                        "{:>4}  {}{flags}  {} B md  {}",
                        hit.score,
                        hit.skill.name,
                        thousands(hit.skill.total_md_bytes),
                        hit.skill.path.display()
                    );
                    for (file, heading) in &hit.matched_headings {
                        println!("        {file} > {heading}");
                    }
                }
            }
        }
    }
    ExitCode::SUCCESS
}
