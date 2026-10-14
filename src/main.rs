use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use girth::config::{Context, resolve_output_dir, resolve_roots};
use girth::ledger::{self, Event, Rating, girth_home};
use girth::measure::{format_table, measure, thousands};
use girth::record::{craft_event, last_craft};
use girth::search::{HitJson, rank};
use girth::skill::CRAFTED_PREFIX;
use girth::skill::{SkillInfo, discover_skills};
use girth::validate::validate;
use std::path::PathBuf;

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
    /// Record an event in the local ledger (used by the plugin hook).
    Record {
        #[command(subcommand)]
        what: RecordWhat,
    },
    /// Summarise the local ledger: crafted skills, uses, removals, ratings.
    Stats {
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Rate a crafted skill: up or down, with an optional private note.
    Feedback {
        skill: String,
        rating: RatingArg,
        /// Stays on this machine; never shared.
        #[arg(long)]
        note: Option<String>,
    },
    /// Check crafted skills against the Girth contract.
    Validate {
        /// Don't record a craft event in the ledger.
        #[arg(long)]
        no_record: bool,
        /// Skill roots used to resolve `src` markers (default: configured roots).
        #[arg(long, num_args = 1..)]
        roots: Vec<String>,
        #[arg(required = true)]
        skill_dirs: Vec<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RecordWhat {
    /// A skill was used. Reads Claude Code hook JSON from stdin unless --skill is given.
    Use {
        #[arg(long)]
        skill: Option<String>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum RatingArg {
    Up,
    Down,
}

/// (skill, project) from a PostToolUse hook payload.
fn hook_use(stdin: &str) -> Option<(String, Option<String>)> {
    let value: serde_json::Value = serde_json::from_str(stdin).ok()?;
    let skill = value.pointer("/tool_input/skill")?.as_str()?.trim();
    let cwd = value
        .get("cwd")
        .and_then(|c| c.as_str())
        .map(str::to_string);
    (!skill.is_empty()).then(|| (skill.to_string(), cwd))
}

fn record_use(skill: Option<String>, ctx: &Context) {
    use std::io::{IsTerminal, Read};
    let found = match skill {
        Some(skill) => Some((skill, None)),
        None if !std::io::stdin().is_terminal() => {
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            hook_use(&input)
        }
        None => None,
    };
    if let Some((skill, cwd)) = found {
        let project = cwd.unwrap_or_else(|| ctx.cwd.display().to_string());
        // Never disturb the session: a failed append is silently dropped.
        let _ = ledger::append(&girth_home(ctx), Event::Use { skill, project });
    }
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

/// Write all output at once; a reader that went away (e.g. `| head`) is not an error.
fn emit(out: &str) {
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(out.as_bytes())
        .and_then(|()| stdout.flush())
    {
        if e.kind() != std::io::ErrorKind::BrokenPipe {
            eprintln!("girth: failed writing output: {e}");
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = Context::from_process();
    let mut warnings = Vec::new();
    let mut out = String::new();
    let code = match cli.command {
        Command::Measure { roots, json } => {
            let roots = resolve_roots(&roots, &ctx, &mut warnings);
            warn(&warnings);
            let skills = discover_skills(&roots);
            report_fallbacks(&skills);
            let report = measure(&skills);
            if json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serialises")
                );
            } else {
                let _ = writeln!(out, "{}", format_table(&report));
            }
            ExitCode::SUCCESS
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
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("hits serialise")
                );
            } else {
                let _ = writeln!(out, "output_dir: {}", output_dir.display());
                if hits.is_empty() {
                    let _ = writeln!(out, "no matching skills");
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
                    let _ = writeln!(
                        out,
                        "{:>4}  {}{flags}  {} B md  {}",
                        hit.score,
                        hit.skill.name,
                        thousands(hit.skill.total_md_bytes),
                        hit.skill.path.display()
                    );
                    for (file, heading) in &hit.matched_headings {
                        let _ = writeln!(out, "        {file} > {heading}");
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Command::Record {
            what: RecordWhat::Use { skill },
        } => {
            record_use(skill, &ctx);
            ExitCode::SUCCESS
        }
        Command::Stats { json } => {
            let home = girth_home(&ctx);
            let stats = girth::stats::compute(&ledger::read(&home), ledger::sharing(&home));
            if json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&stats).expect("stats serialise")
                );
            } else {
                let _ = writeln!(out, "{}", girth::stats::render(&stats));
            }
            ExitCode::SUCCESS
        }
        Command::Feedback {
            skill,
            rating,
            note,
        } => {
            let rating = match rating {
                RatingArg::Up => Rating::Up,
                RatingArg::Down => Rating::Down,
            };
            let project = ctx.cwd.display().to_string();
            let label = if rating == Rating::Up { "👍" } else { "👎" };
            match ledger::append(
                &girth_home(&ctx),
                Event::Feedback {
                    skill: skill.clone(),
                    project,
                    rating,
                    note,
                },
            ) {
                Ok(()) => {
                    let _ = writeln!(out, "recorded {label} for {skill}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("girth: could not record feedback: {e}");
                    ExitCode::from(1)
                }
            }
        }
        Command::Validate {
            no_record,
            roots,
            skill_dirs,
        } => {
            let mut all_valid = true;
            let home = girth_home(&ctx);
            let mut installed: Option<Vec<SkillInfo>> = None;
            for dir in &skill_dirs {
                let name = dir.file_name().unwrap_or_default().to_string_lossy();
                let errors = validate(dir);
                for error in &errors {
                    all_valid = false;
                    let _ = writeln!(out, "INVALID {name}: {error}");
                }
                if !errors.is_empty() {
                    continue;
                }
                if no_record || !name.starts_with(CRAFTED_PREFIX) {
                    let _ = writeln!(out, "VALID {name}");
                    continue;
                }
                let installed = installed.get_or_insert_with(|| {
                    discover_skills(&resolve_roots(&roots, &ctx, &mut warnings))
                });
                let project = ctx.cwd.display().to_string();
                let records = ledger::read(&home);
                let event = craft_event(
                    dir,
                    &project,
                    installed,
                    last_craft(&records, &project, &name),
                );
                let (added, removed) = match &event {
                    Event::Craft { added, removed, .. } => (added.len(), removed.len()),
                    _ => (0, 0),
                };
                match ledger::append(&home, event) {
                    Ok(()) => {
                        let _ =
                            writeln!(out, "VALID {name} (recorded: +{added} −{removed} sections)");
                    }
                    Err(e) => {
                        let _ = writeln!(out, "VALID {name}");
                        eprintln!("girth: could not record to ledger: {e}");
                    }
                }
            }
            warn(&warnings);
            if all_valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    };
    emit(&out);
    code
}
