use std::fmt::Write as _;
use std::io::Write as _;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use girth::config::{Context, resolve_output_dir, resolve_roots};
use girth::ledger::{self, Event, Rating, Record, Sharing, girth_home};
use girth::measure::{format_table, measure, thousands};
use girth::record::{craft_event, last_craft};
use girth::search::{HitJson, rank};
use girth::skill::CRAFTED_PREFIX;
use girth::skill::{SkillInfo, discover_skills};
use girth::taste::{self, TasteEvent, TasteRecord, Tier};
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
    /// Rewrite a Claude Code skill listing (stdin) for this project: keep the
    /// descriptions it needs, collapse the rest into one line of names.
    Lens {
        /// The project dir (default: cwd).
        #[arg(long)]
        project: Option<PathBuf>,
        /// What the text is: the skill listing, MCP server instructions, or the deferred-tools list.
        #[arg(long, value_enum, default_value_t = LensKind::Skills)]
        kind: LensKind,
        /// Print JSON: text, described, collapsed, deferred, bytes_before, bytes_after.
        #[arg(long)]
        json: bool,
    },
    /// Record an event in the local ledger (used by the plugin hook).
    Record {
        #[command(subcommand)]
        what: RecordWhat,
    },
    /// Consent for sharing anonymous metrics, and a preview of exactly what would be shared.
    Share {
        #[arg(value_enum)]
        action: ShareAction,
    },
    /// Summarise the local ledger: crafted skills, uses, removals, ratings, token usage.
    Stats {
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Print only the meter lines (token usage and Lens savings).
        #[arg(long)]
        meter: bool,
        /// Limit the meter to one project dir.
        #[arg(long)]
        project: Option<String>,
        /// Only skill uses and turns from this harness (`claude-code`, `codex`, `copilot`,
        /// `opencode`, `gemini`, `antigravity`); default all.
        #[arg(long)]
        harness: Option<String>,
    },
    /// What one request was made of: reads the mod's snapshot JSON from stdin.
    Xray {
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Token usage per turn (used by the plugin's mod).
    Meter {
        #[command(subcommand)]
        what: MeterWhat,
    },
    /// Keep the prompt cache warm through idle gaps (used by the plugin's mod).
    Keeper {
        #[command(subcommand)]
        what: KeeperWhat,
    },
    /// Claude Code's requests outside turns, such as progress summaries (used by the plugin's mod).
    Side {
        #[command(subcommand)]
        what: SideWhat,
    },
    /// The MCP tool schemas a session's requests carry (used by the plugin's mod).
    Schema {
        #[command(subcommand)]
        what: SchemaWhat,
    },
    /// Idle compactions of large conversations (used by the plugin's mod).
    Compact {
        #[command(subcommand)]
        what: CompactWhat,
    },
    /// The settings the plugin's mod reads at session start, as JSON.
    Settings {
        /// The project directory (default: the current directory).
        #[arg(long)]
        project: Option<String>,
    },
    /// Rate a crafted skill: up or down, with an optional private note.
    Feedback {
        skill: String,
        rating: RatingArg,
        /// Stays on this machine; never shared.
        #[arg(long)]
        note: Option<String>,
    },
    /// House rules and recent choices per domain (local only, never shared).
    Taste {
        #[command(subcommand)]
        action: Option<TasteAction>,
    },
    /// Remember what a finished deliverable chose, so the next one can vary it.
    Made {
        /// Domain slug, e.g. launch-video.
        domain: String,
        /// Choices as key=value, e.g. palette=neon-on-black.
        #[arg(required = true)]
        choices: Vec<String>,
    },
    /// Print a skill's frontmatter and contents, or only the sections you name.
    Show {
        /// The skill dir (as printed by `search`).
        dir: PathBuf,
        /// A heading to print with its subsections; repeatable.
        #[arg(long = "heading")]
        headings: Vec<String>,
    },
    /// Shallow-clone a public GitHub skill repo (owner/repo[/path]) into an untrusted scratch dir.
    Fetch {
        /// owner/repo or owner/repo/sub/path.
        spec: String,
    },
    /// What is costing you, the lever that fixes it, and why the fix keeps things working
    Advise {
        /// Window: `24h`, `7d`, or seconds (default 7d).
        #[arg(long, default_value = "7d")]
        since: String,
        /// Only the top contributor, in one line, and only when it is worth saying.
        #[arg(long)]
        brief: bool,
        #[arg(long)]
        json: bool,
        /// Only turns from this harness (`claude-code`, `codex`, `copilot`, `opencode`, `gemini`, `antigravity`); default all, or
        /// `claude-code` with --brief, which Claude Code shows at session start.
        #[arg(long)]
        harness: Option<String>,
    },
    /// Report source sections that changed since each crafted skill was validated (exit 1 if any)
    Drift {
        /// Skill roots holding the sources (default: configured roots).
        #[arg(long, num_args = 1..)]
        roots: Vec<String>,
        #[arg(required = true)]
        skill_dirs: Vec<PathBuf>,
    },
    /// Append spend from other harnesses' local session logs (Codex, Copilot CLI, OpenCode, Gemini CLI) to the ledger.
    Import {
        /// Which harness: `codex`, `copilot`, `opencode`, `gemini`, `antigravity` or `all`.
        #[arg(long, default_value = "all")]
        from: String,
    },
    /// Run tasks with and without a skill, grade each run with a check command, and record whether the skill earned its place.
    ///
    /// Each run works in a fresh clone of this repo at HEAD. A task counts as won or lost only by a difference of 2 or more
    /// passes, so one flipped run never decides: a skill that adds a single pass on every task stays undecided until more
    /// runs settle it. Cost is shown, never judged. The default agent is Claude Code, headless, with user-level skills,
    /// plugins and MCP servers hidden; `--agent` takes any command that reads the prompt on stdin. Runs spend model usage.
    Trial {
        /// The skill folder (with a SKILL.md), usually a crafted girth-* skill.
        skill_dir: PathBuf,
        /// A prompt file; repeatable.
        #[arg(long = "task", required = true)]
        tasks: Vec<PathBuf>,
        /// A shell command run in the work tree after the agent: exit 0 passes. One per --task, or one for all.
        #[arg(long = "check", required = true)]
        checks: Vec<String>,
        /// Runs per arm and task (at least 2).
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u64).range(2..))]
        runs: u64,
        /// The agent command; it gets the prompt on stdin. Default: headless Claude Code.
        #[arg(long)]
        agent: Option<String>,
        /// The model for the default agent.
        #[arg(long)]
        model: Option<String>,
        /// Seconds before a hung agent run is killed and counted as a fail.
        #[arg(long, default_value_t = 1800)]
        timeout: u64,
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
enum TasteAction {
    /// House rules and recent choices for one domain.
    Show {
        domain: String,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remember a house rule (personal tier unless --project).
    Add {
        domain: String,
        text: String,
        /// Store in this project's .girth/taste.jsonl instead.
        #[arg(long)]
        project: bool,
    },
    /// Forget rule N, as numbered by `taste show`.
    Drop {
        domain: String,
        n: usize,
        /// The rule is a project rule.
        #[arg(long)]
        project: bool,
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

#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum LensKind {
    Skills,
    Mcp,
    Tools,
}

#[derive(Subcommand)]
enum MeterWhat {
    /// Append one turn's usage, read as JSON from stdin, and print the keeper's
    /// plan and any cache-break line as JSON. Bad input is ignored.
    Record,
}

#[derive(Subcommand)]
enum CompactWhat {
    /// Append one idle compaction, read as JSON from stdin. Bad input is ignored.
    Record,
}

#[derive(Subcommand)]
enum SchemaWhat {
    /// Append one session's schema tokens per server, read as JSON from stdin. Bad input is ignored.
    Record,
}

#[derive(Subcommand)]
enum SideWhat {
    /// Append side-query totals, read as a JSON array from stdin. Bad items are ignored.
    Record,
}

#[derive(Subcommand)]
enum KeeperWhat {
    /// Append one ping's usage, read as JSON from stdin. Bad input is ignored.
    Record,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ShareAction {
    Status,
    Allow,
    Deny,
    Preview,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum RatingArg {
    Up,
    Down,
}

/// A skill use as a PostToolUse hook reports it.
struct HookUse {
    skill: String,
    cwd: Option<String>,
    session: Option<String>,
    agent: bool,
}

fn hook_use(stdin: &str) -> Option<HookUse> {
    let value: serde_json::Value = serde_json::from_str(stdin).ok()?;
    let skill = value.pointer("/tool_input/skill")?.as_str()?.trim();
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    (!skill.is_empty()).then(|| HookUse {
        skill: skill.to_string(),
        cwd: text("cwd"),
        session: text("session_id"),
        agent: text("agent_id").is_some(),
    })
}

/// Record a use; for a skill with remembered taste, return the hook JSON that injects it.
fn record_use(skill: Option<String>, ctx: &Context) -> String {
    use std::io::{IsTerminal, Read};
    let found = match skill {
        Some(skill) => Some(HookUse {
            skill,
            cwd: None,
            session: None,
            agent: false,
        }),
        None if !std::io::stdin().is_terminal() => {
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            hook_use(&input)
        }
        None => None,
    };
    let Some(HookUse {
        skill,
        cwd,
        session,
        agent,
    }) = found
    else {
        return String::new();
    };
    let project = cwd.unwrap_or_else(|| ctx.cwd.display().to_string());
    let home = girth_home(ctx);
    // Never disturb the session: a failed append is silently dropped.
    let _ = ledger::append(
        &home,
        Event::Use {
            skill: skill.clone(),
            project: project.clone(),
            session,
            agent,
            harness: None,
        },
    );
    // The hook runs on every Skill call: parse only lines that can matter.
    let project = std::path::Path::new(&project);
    let files = [Tier::Personal, Tier::Project].map(|t| taste::path(t, &home, project));
    let name = skill.rsplit(':').next().unwrap_or(&skill);
    let links: Vec<TasteRecord> = files
        .iter()
        .flat_map(|f| taste::read_matching(f, &[name]))
        .collect();
    let domains = taste::resolve_domains(&skill, &links);
    let needles: Vec<&str> = domains.iter().map(String::as_str).collect();
    let [personal, project_tier] = files.map(|f| taste::read_matching(&f, &needles));
    let views: Vec<_> = domains
        .iter()
        .map(|d| taste::view(&personal, &project_tier, d))
        .filter(|v| !v.is_empty())
        .collect();
    if views.is_empty() {
        return String::new();
    }
    let budget = HOOK_CONTEXT_MAX / views.len();
    let context: Vec<String> = views
        .iter()
        .map(|v| taste::render_capped(v, &launcher(ctx), budget))
        .collect();
    let payload = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": context.join("\n"),
        }
    });
    format!("{payload}\n")
}

const HOOK_CONTEXT_MAX: usize = 1500;

fn taste_tiers(
    home: &std::path::Path,
    project: &std::path::Path,
) -> (Vec<TasteRecord>, Vec<TasteRecord>) {
    (
        taste::read(&taste::path(Tier::Personal, home, project)),
        taste::read(&taste::path(Tier::Project, home, project)),
    )
}

/// How the model should invoke girth: the plugin launcher when it exported itself.
fn launcher(ctx: &Context) -> String {
    ctx.env
        .get("GIRTH_LAUNCHER")
        .filter(|v| !v.is_empty())
        .cloned()
        .unwrap_or_else(|| "girth".to_string())
}

fn check_domain(domain: &str) -> Result<(), ExitCode> {
    if taste::is_domain(domain) {
        Ok(())
    } else {
        eprintln!(
            "girth: invalid domain '{domain}' (use a lowercase hyphenated slug, e.g. launch-video)"
        );
        Err(ExitCode::from(2))
    }
}

fn run_taste(action: Option<TasteAction>, ctx: &Context, out: &mut String) -> ExitCode {
    let home = girth_home(ctx);
    let (personal, project) = taste_tiers(&home, &ctx.cwd);
    let Some(action) = action else {
        let rows = taste::domains(&personal, &project);
        if rows.is_empty() {
            let _ = writeln!(out, "no taste remembered yet");
        }
        for row in rows {
            let _ = writeln!(
                out,
                "{}  {} rules, {} made",
                row.domain, row.rules, row.made
            );
        }
        return ExitCode::SUCCESS;
    };
    match action {
        TasteAction::Show { domain, json } => {
            if let Err(code) = check_domain(&domain) {
                return code;
            }
            let view = taste::view(&personal, &project, &domain);
            if json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&view).expect("view serialises")
                );
            } else {
                out.push_str(&taste::render(&view, &launcher(ctx)));
            }
            ExitCode::SUCCESS
        }
        TasteAction::Add {
            domain,
            text,
            project: in_project,
        } => {
            if let Err(code) = check_domain(&domain) {
                return code;
            }
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                eprintln!("girth: a rule needs some text");
                return ExitCode::from(2);
            }
            let tier = if in_project {
                Tier::Project
            } else {
                Tier::Personal
            };
            let event = TasteEvent::Rule {
                domain: domain.clone(),
                text: text.clone(),
                op: taste::Op::Add,
            };
            match taste::append(&taste::path(tier, &home, &ctx.cwd), event) {
                Ok(()) => {
                    let _ = writeln!(out, "remembered for {domain}: {text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("girth: could not write taste: {e}");
                    ExitCode::from(1)
                }
            }
        }
        TasteAction::Drop {
            domain,
            n,
            project: in_project,
        } => {
            if let Err(code) = check_domain(&domain) {
                return code;
            }
            let view = taste::view(&personal, &project, &domain);
            let Some(row) = view.rules.iter().find(|r| r.n == n) else {
                eprintln!("girth: no rule {n} in {domain}");
                return ExitCode::from(1);
            };
            let tier = if in_project {
                Tier::Project
            } else {
                Tier::Personal
            };
            if row.tier != tier {
                match row.tier {
                    Tier::Project => eprintln!("girth: rule {n} is a project rule (add --project)"),
                    Tier::Personal => {
                        eprintln!("girth: rule {n} is a personal rule (drop --project)")
                    }
                }
                return ExitCode::from(1);
            }
            let event = TasteEvent::Rule {
                domain: domain.clone(),
                text: row.text.clone(),
                op: taste::Op::Drop,
            };
            match taste::append(&taste::path(tier, &home, &ctx.cwd), event) {
                Ok(()) => {
                    let _ = writeln!(out, "dropped from {domain}: {}", row.text);
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("girth: could not write taste: {e}");
                    ExitCode::from(1)
                }
            }
        }
    }
}

fn run_made(domain: &str, pairs: &[String], ctx: &Context, out: &mut String) -> ExitCode {
    if let Err(code) = check_domain(domain) {
        return code;
    }
    let mut choices = std::collections::BTreeMap::new();
    for pair in pairs {
        match taste::parse_choice(pair) {
            Ok((key, value)) => {
                choices.insert(key, value);
            }
            Err(e) => {
                eprintln!("girth: {e}");
                return ExitCode::from(2);
            }
        }
    }
    let count = choices.len();
    let event = TasteEvent::Made {
        domain: domain.to_string(),
        project: ctx.cwd.display().to_string(),
        choices,
    };
    let home = girth_home(ctx);
    match taste::append(&taste::path(Tier::Personal, &home, &ctx.cwd), event) {
        Ok(()) => {
            let _ = writeln!(out, "remembered {count} choices for {domain}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("girth: could not write taste: {e}");
            ExitCode::from(1)
        }
    }
}

/// Link each source skill of a recorded craft to its domain, once per pair.
fn link_sources(home: &std::path::Path, ctx: &Context, crafted: &str, event: &Event) {
    let (Some(domain), Event::Craft { sources, .. }) =
        (crafted.strip_prefix(CRAFTED_PREFIX), event)
    else {
        return;
    };
    let file = taste::path(Tier::Personal, home, &ctx.cwd);
    let existing = taste::read(&file);
    for source in sources {
        let known = existing.iter().any(|r| {
            matches!(&r.event, TasteEvent::Link { domain: d, skill } if d == domain && *skill == source.skill)
        });
        if !known {
            let _ = taste::append(
                &file,
                TasteEvent::Link {
                    domain: domain.to_string(),
                    skill: source.skill.clone(),
                },
            );
        }
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

fn run_lens(
    project: Option<PathBuf>,
    kind: LensKind,
    json: bool,
    ctx: &Context,
    out: &mut String,
) -> ExitCode {
    use girth::mcp_lens::{self, McpRewrite};
    use std::io::Read;
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    let project_dir = project.unwrap_or_else(|| ctx.cwd.clone());
    let home = girth_home(ctx);
    let (enabled, keep) = girth::lens::settings(&project_dir, &home);
    let kind_name = match kind {
        LensKind::Skills => "skills",
        LensKind::Mcp => "mcp",
        LensKind::Tools => "tools",
    };
    // GIRTH_LENS_KINDS (comma-separated) limits which texts Lens rewrites.
    let kind_off = ctx
        .env
        .get("GIRTH_LENS_KINDS")
        .is_some_and(|v| !v.split(',').any(|k| k.trim() == kind_name));
    let off = kind_off || ctx.env.get("GIRTH_LENS").is_some_and(|v| v == "off");
    let records = ledger::read(&home);
    let rewrite = if off || !enabled {
        McpRewrite {
            text: text.clone(),
            ..Default::default()
        }
    } else {
        let names: Vec<String> = match kind {
            LensKind::Skills => girth::lens::parse(&text)
                .into_iter()
                .filter_map(|i| match i {
                    girth::lens::Item::Entry { name, .. } => Some(name),
                    girth::lens::Item::Raw(_) => None,
                })
                .collect(),
            LensKind::Mcp | LensKind::Tools => mcp_lens::server_ids(&text)
                .into_iter()
                .map(|id| format!("{}{id}", girth::lens::SERVER_PREFIX))
                .collect(),
        };
        let unseen = girth::lens::unseen(&records, &names);
        let project = project_dir.display().to_string();
        let inputs = girth::lens::Inputs {
            records: &records,
            project: &project,
            project_dir: &project_dir,
            keep: &keep,
            now: ledger::now(),
        };
        let rewrite = match kind {
            LensKind::Skills => {
                let r = girth::lens::rewrite(&text, &inputs);
                McpRewrite {
                    text: r.text,
                    described: r.described,
                    collapsed: r.collapsed,
                    ..Default::default()
                }
            }
            LensKind::Mcp => mcp_lens::rewrite_instructions(&text, &inputs),
            LensKind::Tools => mcp_lens::rewrite_tools(&text, &inputs),
        };
        if !unseen.is_empty() {
            // Never disturb the session: a failed append is dropped.
            let _ = ledger::append(&home, Event::Seen { names: unseen });
        }
        rewrite
    };
    if json {
        let payload = serde_json::json!({
            "text": rewrite.text,
            "described": rewrite.described,
            "collapsed": rewrite.collapsed,
            "deferred": rewrite.deferred,
            "bytes_before": text.len(),
            "bytes_after": rewrite.text.len(),
        });
        let _ = writeln!(out, "{payload}");
    } else {
        out.push_str(&rewrite.text);
    }
    ExitCode::SUCCESS
}

/// Keep turns and skill uses from harness `h` (`claude-code` for the mod's own records);
/// crafts, ratings and other events belong to no harness and stay.
fn only_harness(records: &mut Vec<Record>, h: &str) {
    records.retain(|r| match &r.event {
        Event::Turn(t) => t.harness.as_deref().unwrap_or("claude-code") == h,
        Event::Use { harness, .. } => harness.as_deref().unwrap_or("claude-code") == h,
        Event::Side(_) | Event::Schema(_) => h == "claude-code",
        // Trials, crafts, feedback: not tied to the harness that spends.
        _ => true,
    });
}

#[allow(clippy::too_many_arguments)]
fn run_trial(
    skill_dir: PathBuf,
    tasks: Vec<PathBuf>,
    checks: Vec<String>,
    runs: u64,
    agent: Option<String>,
    model: Option<String>,
    timeout: u64,
    ctx: &Context,
    out: &mut String,
) -> ExitCode {
    let refuse = |msg: String| {
        eprintln!("girth trial: {msg}");
        ExitCode::from(2)
    };
    if checks.len() != 1 && checks.len() != tasks.len() {
        return refuse("give one --check for every --task, or a single one for all".into());
    }
    let skill_dir = ctx.cwd.join(skill_dir);
    if !skill_dir.join("SKILL.md").is_file() {
        return refuse(format!("{} has no SKILL.md", skill_dir.display()));
    }
    let top = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&ctx.cwd)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
    let Some(top) = top else {
        return refuse(
            "run it inside a git repository with a commit: each run starts from a clone at HEAD"
                .into(),
        );
    };
    let mut pairs = Vec::new();
    for (i, task) in tasks.iter().enumerate() {
        match std::fs::read_to_string(ctx.cwd.join(task)) {
            Ok(prompt) => pairs.push((
                prompt,
                checks[if checks.len() == 1 { 0 } else { i }].clone(),
            )),
            Err(e) => return refuse(format!("{}: {e}", task.display())),
        }
    }
    let agent = agent.unwrap_or_else(|| match model {
        Some(m) => format!("{} --model {m}", girth::trial::DEFAULT_AGENT),
        None => girth::trial::DEFAULT_AGENT.to_string(),
    });
    let opts = girth::trial::Options {
        skill_dir,
        tasks: pairs,
        runs,
        agent,
        timeout: std::time::Duration::from_secs(timeout),
    };
    let report = match girth::trial::run(&top, &opts, |line| eprintln!("girth trial: {line}")) {
        Ok(r) => r,
        Err(e) => return refuse(e),
    };
    let micros = |c: Option<f64>| c.map(|c| (c * 1e6).round() as u64);
    let _ = ledger::append(
        &girth_home(ctx),
        Event::Trial(girth::ledger::Trial {
            project: ctx.cwd.display().to_string(),
            skill: report.skill.clone(),
            tasks: report.tasks.clone(),
            fired: report.fired,
            with_cost_micros: micros(report.with_cost),
            without_cost_micros: micros(report.without_cost),
            verdict: report.verdict,
        }),
    );
    let _ = writeln!(out, "{}", report.render());
    ExitCode::SUCCESS
}

fn run_stats(
    json: bool,
    meter: bool,
    project: Option<String>,
    harness: Option<&str>,
    ctx: &Context,
    out: &mut String,
) -> ExitCode {
    let home = girth_home(ctx);
    let mut records = ledger::read(&home);
    if let Some(h) = harness {
        only_harness(&mut records, h);
    }
    let summary = girth::meter::summarise(&records, project.as_deref());
    let keeper = girth::keeper::summarise(&records, project.as_deref());
    let keeper_text = girth::keeper::render(&keeper);
    if meter && !json {
        let mut text = girth::meter::render(&summary);
        if !keeper_text.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&keeper_text);
        }
        if text.is_empty() {
            let _ = writeln!(*out, "meter: no turns recorded yet");
        } else {
            let _ = writeln!(*out, "{text}");
        }
        return ExitCode::SUCCESS;
    }
    let stats = girth::stats::compute(&records, ledger::sharing(&home));
    let (personal, project) = taste_tiers(&home, &ctx.cwd);
    let domains = taste::domains(&personal, &project);
    let taste_counts = (!domains.is_empty()).then(|| {
        (
            domains.len(),
            domains.iter().map(|d| d.rules).sum::<usize>(),
            domains.iter().map(|d| d.made).sum::<usize>(),
        )
    });
    if json {
        let mut value = serde_json::to_value(&stats).expect("stats serialise");
        if let Some((d, r, m)) = taste_counts {
            value["taste"] = serde_json::json!({"domains": d, "rules": r, "made": m});
        }
        if summary.turns > 0 || summary.side_requests > 0 {
            value["meter"] = serde_json::to_value(&summary).expect("meter serialises");
        }
        if keeper.breaks > 0 || keeper.pings > 0 || keeper.compactions > 0 {
            value["keeper"] = serde_json::to_value(&keeper).expect("keeper serialises");
        }
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).expect("stats serialise")
        );
    } else {
        let _ = writeln!(*out, "{}", girth::stats::render(&stats));
        if let Some((d, r, m)) = taste_counts {
            let _ = writeln!(*out, "taste: {d} domains, {r} rules, {m} made");
        }
        let meter = girth::meter::render(&summary);
        if !meter.is_empty() {
            let _ = writeln!(*out, "{meter}");
        }
        if !keeper_text.is_empty() {
            let _ = writeln!(*out, "{keeper_text}");
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = Context::from_process();
    let mut warnings = Vec::new();
    let mut out = String::new();
    let code = match cli.command {
        Command::Lens {
            project,
            kind,
            json,
        } => run_lens(project, kind, json, &ctx, &mut out),
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
            out.push_str(&record_use(skill, &ctx));
            ExitCode::SUCCESS
        }
        Command::Taste { action } => run_taste(action, &ctx, &mut out),
        Command::Show { dir, headings } => match girth::show::show(&dir, &headings) {
            Ok(text) => {
                out.push_str(&text);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("girth: {e}");
                ExitCode::from(1)
            }
        },
        Command::Fetch { spec } => match girth::fetch::parse_spec(&spec) {
            Err(e) => {
                eprintln!("girth: {e}");
                ExitCode::from(2)
            }
            Ok(spec) => {
                let git = ctx
                    .env
                    .get(girth::fetch::GIT_ENV)
                    .filter(|g| !g.is_empty())
                    .map_or("git", String::as_str);
                match girth::fetch::fetch(&spec, &girth_home(&ctx), git) {
                    Ok(path) => {
                        let _ = writeln!(out, "fetched: {}", path.display());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("girth: {e}");
                        ExitCode::from(1)
                    }
                }
            }
        },
        Command::Made { domain, choices } => run_made(&domain, &choices, &ctx, &mut out),
        Command::Share { action } => {
            let home = girth_home(&ctx);
            let set = |value| ledger::set_sharing(&home, value);
            let result = match action {
                ShareAction::Status => Ok(()),
                ShareAction::Allow => set(Sharing::Allowed),
                ShareAction::Deny => set(Sharing::Denied),
                ShareAction::Preview => {
                    eprintln!(
                        "girth: nothing is uploaded in v{}; this is exactly what would be shared.",
                        env!("CARGO_PKG_VERSION")
                    );
                    let payload = girth::share::payload(&ledger::read(&home));
                    let _ = writeln!(
                        out,
                        "{}",
                        serde_json::to_string_pretty(&payload).expect("payload serialises")
                    );
                    Ok(())
                }
            };
            match result {
                Err(e) => {
                    eprintln!("girth: could not save sharing choice: {e}");
                    ExitCode::from(1)
                }
                Ok(()) => {
                    if !matches!(action, ShareAction::Preview) {
                        let label = girth::stats::sharing_label(ledger::sharing(&home));
                        let _ = writeln!(out, "sharing: {label}");
                    }
                    ExitCode::SUCCESS
                }
            }
        }
        Command::Xray { json } => {
            use std::io::Read;
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            let snapshot: girth::xray::Snapshot = serde_json::from_str(&input).unwrap_or_default();
            let report = girth::xray::report(&snapshot);
            if json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&report).expect("report serialises")
                );
            } else {
                let _ = writeln!(out, "{}", girth::xray::render(&report));
            }
            ExitCode::SUCCESS
        }
        Command::Meter {
            what: MeterWhat::Record,
        } => {
            use std::io::Read;
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            if let Ok(mut turn) = serde_json::from_str::<ledger::Turn>(&input) {
                // The plan's fields parse on their own, so a malformed `bash`
                // list costs the turn its outcomes and nothing else.
                #[derive(serde::Deserialize)]
                struct Compacted {
                    before: u64,
                    after: Option<u64>,
                }
                #[derive(serde::Deserialize, Default)]
                struct Extra {
                    #[serde(default)]
                    context: u64,
                    compacted: Option<Compacted>,
                }
                let extra = serde_json::from_str::<Extra>(&input).unwrap_or_default();
                let context = extra.context;
                let compacted = extra
                    .compacted
                    .map(|c| girth::keeper::compact_line(c.before, c.after));
                // Classified here and dropped: command text never reaches the ledger.
                let calls: Vec<girth::outcome::Call> =
                    serde_json::from_str::<serde_json::Value>(&input)
                        .ok()
                        .and_then(|v| v.get("bash")?.as_array().cloned())
                        .unwrap_or_default()
                        .into_iter()
                        .take(girth::outcome::MAX_CALLS)
                        .filter_map(|c| serde_json::from_value(c).ok())
                        .collect();
                turn.outcomes = girth::outcome::outcomes(&calls);
                let home = girth_home(&ctx);
                let settings = girth::keeper::settings(std::path::Path::new(&turn.project), &home);
                let keeper_off =
                    turn.agent || ctx.env.get("GIRTH_KEEPER").is_some_and(|v| v == "off");
                let compact_off =
                    turn.agent || ctx.env.get("GIRTH_COMPACT").is_some_and(|v| v == "off");
                let interval = ctx
                    .env
                    .get("GIRTH_KEEPER_INTERVAL_MS")
                    .and_then(|v| v.parse().ok());
                #[derive(serde::Serialize)]
                struct Report {
                    keeper: girth::keeper::Plan,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    line: Option<String>,
                }
                let report = Report {
                    keeper: girth::keeper::plan(
                        &settings,
                        keeper_off,
                        compact_off,
                        interval,
                        context,
                    ),
                    line: match (compacted, girth::keeper::doctor_line(&turn)) {
                        (Some(c), Some(d)) => Some(format!("{c}\n{d}")),
                        (c, d) => c.or(d),
                    },
                };
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&report).expect("report serialises")
                );
                // Never disturb the session: a failed append is dropped.
                let _ = ledger::append(&home, Event::Turn(turn));
            }
            ExitCode::SUCCESS
        }
        Command::Compact {
            what: CompactWhat::Record,
        } => {
            use std::io::Read;
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            if let Ok(compact) = serde_json::from_str::<ledger::Compact>(&input) {
                let _ = ledger::append(&girth_home(&ctx), Event::Compact(compact));
            }
            ExitCode::SUCCESS
        }
        Command::Settings { project } => {
            let dir = project.map_or_else(|| ctx.cwd.clone(), std::path::PathBuf::from);
            let mut value = serde_json::json!({});
            if let Some(model) = girth::keeper::agents_model(&dir, &girth_home(&ctx)) {
                value["agents"] = serde_json::json!({ "model": model });
            }
            let _ = writeln!(out, "{value}");
            ExitCode::SUCCESS
        }
        Command::Side {
            what: SideWhat::Record,
        } => {
            use std::io::Read;
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            let items = serde_json::from_str::<Vec<serde_json::Value>>(&input).unwrap_or_default();
            let home = girth_home(&ctx);
            for item in items {
                if let Ok(side) = serde_json::from_value::<ledger::Side>(item) {
                    let _ = ledger::append(&home, Event::Side(side));
                }
            }
            ExitCode::SUCCESS
        }
        Command::Schema {
            what: SchemaWhat::Record,
        } => {
            use std::io::Read;
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            if let Ok(schema) = serde_json::from_str::<ledger::Schema>(&input) {
                let _ = ledger::append(&girth_home(&ctx), Event::Schema(schema));
            }
            ExitCode::SUCCESS
        }
        Command::Keeper {
            what: KeeperWhat::Record,
        } => {
            use std::io::Read;
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            if let Ok(ping) = serde_json::from_str::<ledger::Ping>(&input) {
                let _ = ledger::append(&girth_home(&ctx), Event::Ping(ping));
            }
            ExitCode::SUCCESS
        }
        Command::Stats {
            json,
            meter,
            project,
            harness,
        } => run_stats(json, meter, project, harness.as_deref(), &ctx, &mut out),
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
        Command::Import { from } => {
            use girth::import::SOURCES;
            if from != "all" && !SOURCES.contains(&from.as_str()) {
                eprintln!(
                    "girth: unknown harness: {from} (try codex, copilot, opencode, gemini, antigravity or all)"
                );
                return ExitCode::from(2);
            }
            let var = |k: &str| ctx.env.get(k).filter(|v| !v.is_empty()).map(PathBuf::from);
            let under_home = |p: &str| ctx.home.as_ref().map(|h| h.join(p));
            let opencode_bin = var("GIRTH_OPENCODE_BIN").unwrap_or_else(|| "opencode".into());
            for name in SOURCES.into_iter().filter(|n| from == "all" || from == *n) {
                let g = girth_home(&ctx);
                let result = match name {
                    "codex" => var("CODEX_HOME")
                        .or_else(|| under_home(".codex"))
                        .map(|h| girth::import::import_codex(&h, &g)),
                    "copilot" => var("COPILOT_HOME")
                        .or_else(|| under_home(".copilot"))
                        .map(|h| girth::import::import_copilot(&h, &g)),
                    "gemini" => var("GEMINI_CLI_HOME")
                        .or_else(|| ctx.home.clone())
                        .map(|h| girth::import::import_gemini(&h.join(".gemini"), &g)),
                    "antigravity" => var("GEMINI_CLI_HOME")
                        .or_else(|| ctx.home.clone())
                        .map(|h| {
                            let sqlite3 =
                                var("GIRTH_SQLITE3_BIN").unwrap_or_else(|| "sqlite3".into());
                            let dir = h.join(".gemini/antigravity-cli/conversations");
                            girth::import::import_antigravity(&sqlite3, &dir, &g, ledger::now())
                        }),
                    _ => var("XDG_DATA_HOME")
                        .or_else(|| under_home(".local/share"))
                        .map(|d| {
                            let db = d.join("opencode/opencode.db");
                            let sqlite3 =
                                var("GIRTH_SQLITE3_BIN").unwrap_or_else(|| "sqlite3".into());
                            girth::import::import_opencode_with(&opencode_bin, &sqlite3, &db, &g)
                        }),
                };
                let n = match result {
                    Some(Err(e)) => {
                        eprintln!("girth: import from {name} failed: {e}");
                        return ExitCode::FAILURE;
                    }
                    Some(Ok(n)) => n,
                    None => 0,
                };
                let _ = writeln!(out, "imported {n} turns from {name}");
            }
            ExitCode::SUCCESS
        }
        Command::Advise {
            since,
            brief,
            json,
            harness,
        } => {
            let window = match since.strip_suffix('d') {
                Some(d) => d.parse::<u64>().map(|d| d * 86_400),
                None => match since.strip_suffix('h') {
                    Some(h) => h.parse::<u64>().map(|h| h * 3600),
                    None => since.parse::<u64>(),
                },
            };
            let Ok(window) = window else {
                eprintln!("girth: --since takes 24h, 7d or seconds, got '{since}'");
                return ExitCode::from(2);
            };
            let home = girth_home(&ctx);
            let keeper = girth::keeper::settings(&ctx.cwd, &home);
            let on = girth::advise::On {
                keeper: keeper.enabled,
                compact_idle: keeper.compact_idle,
                agents_model: girth::keeper::agents_model(&ctx.cwd, &home).is_some(),
                harness: girth::import::SOURCES
                    .into_iter()
                    .find(|s| harness.as_deref() == Some(*s)),
            };
            let harness = harness.or_else(|| brief.then(|| "claude-code".to_string()));
            let mut records = ledger::read(&home);
            if let Some(h) = &harness {
                only_harness(&mut records, h);
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let advice = girth::advise::advise(&records, now, window, on);
            if json {
                let _ = writeln!(
                    out,
                    "{}",
                    serde_json::to_string(&advice).unwrap_or_default()
                );
            } else if brief {
                if let Some(line) = girth::advise::brief(&advice) {
                    let _ = writeln!(out, "{line}");
                }
            } else {
                let _ = write!(out, "{}", girth::advise::render(&advice));
            }
            ExitCode::SUCCESS
        }
        Command::Trial {
            skill_dir,
            tasks,
            checks,
            runs,
            agent,
            model,
            timeout,
        } => run_trial(
            skill_dir, tasks, checks, runs, agent, model, timeout, &ctx, &mut out,
        ),
        Command::Drift { roots, skill_dirs } => {
            let installed = discover_skills(&resolve_roots(&roots, &ctx, &mut warnings));
            let records = ledger::read(&girth_home(&ctx));
            let project = ctx.cwd.display().to_string();
            let mut clean = true;
            for dir in &skill_dirs {
                let name = dir.file_name().unwrap_or_default().to_string_lossy();
                let Some(craft) = last_craft(&records, &project, &name) else {
                    clean = false;
                    let _ = writeln!(
                        out,
                        "{name}: no recorded craft here (run girth validate first)"
                    );
                    continue;
                };
                let findings = girth::drift::drift(craft, &installed);
                if findings.is_empty() {
                    let _ = writeln!(out, "{name}: up to date");
                }
                for finding in &findings {
                    clean = false;
                    let _ = writeln!(out, "{name}: {finding}");
                }
            }
            warn(&warnings);
            if clean {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
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
                if name.starts_with(CRAFTED_PREFIX) {
                    // One cache for every harness: Claude Code reads .claude/skills, most
                    // others .agents/skills. A failed link never fails validation.
                    match girth::link::link_other_side(dir) {
                        Ok(Some(at)) => eprintln!(
                            "girth: linked {} so other harnesses load it too",
                            at.display()
                        ),
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("girth: could not link {name} for other harnesses: {e}")
                        }
                    }
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
                link_sources(&home, &ctx, &name, &event);
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
