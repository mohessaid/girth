//! Lens for MCP: the server instructions and deferred-tool names Claude Code
//! sends on every request. Servers this project doesn't use collapse to one
//! line each; their instructions are handed to the mod for first-use delivery.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ledger::Event;
use crate::lens::{Inputs, RECENT_SECS, SERVER_PREFIX, first_seen, is_new};

/// Frozen: the line naming servers whose instructions were held back.
pub const OTHER_SERVERS: &str =
    "## Other connected servers (their instructions arrive on first use): ";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpRewrite {
    pub text: String,
    /// Server ids kept in full / collapsed, in input order.
    pub described: Vec<String>,
    pub collapsed: Vec<String>,
    /// Collapsed server id → its instructions section, for first-use delivery.
    pub deferred: BTreeMap<String, String>,
}

/// The tool-prefix id for an instructions heading: runs of characters other
/// than ASCII letters and digits become `_` (`claude.ai Figma` → `claude_ai_Figma`).
pub fn server_id(heading: &str) -> String {
    let mut id = String::with_capacity(heading.len());
    for c in heading.trim().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    id.trim_matches('_').to_string()
}

/// The server id of a tool name `mcp__<id>__<tool>`.
pub fn tool_server(tool: &str) -> Option<&str> {
    let rest = tool.strip_prefix("mcp__")?;
    let end = rest.find("__")?;
    (end > 0).then(|| &rest[..end])
}

fn heading(line: &str) -> Option<&str> {
    line.strip_prefix("## ")
        .map(|h| h.trim_end_matches(['\n', '\r']))
}

/// Server ids present in an instructions or deferred-tools text, in order.
pub fn server_ids(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for line in text.lines() {
        let id = match heading(line) {
            Some(h) => server_id(h),
            None => match tool_server(line.trim()) {
                Some(id) => id.to_string(),
                None => continue,
            },
        };
        if !id.is_empty() && !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// Which servers this project needs, from ledger evidence.
struct Evidence<'a> {
    inputs: &'a Inputs<'a>,
    baseline: Option<u64>,
    first: HashMap<&'a str, u64>,
    used: HashSet<&'a str>,
}

impl<'a> Evidence<'a> {
    fn new(inputs: &'a Inputs<'a>) -> Self {
        let (baseline, first) = first_seen(inputs.records, true);
        let mut used = HashSet::new();
        for record in inputs.records {
            if let Event::Turn(t) = &record.event {
                let recent = inputs.now.saturating_sub(record.ts) < RECENT_SECS;
                if t.project == inputs.project || recent {
                    used.extend(t.mcp.iter().map(String::as_str));
                }
            }
        }
        Self {
            inputs,
            baseline,
            first,
            used,
        }
    }

    fn keeps(&self, id: &str) -> bool {
        let key = format!("{SERVER_PREFIX}{id}");
        let new = is_new(self.baseline, self.first.get(key.as_str()), self.inputs.now);
        new || self.used.contains(id) || self.inputs.keep.iter().any(|k| k == id)
    }
}

/// Rewrite the `mcp_instructions_delta` text.
pub fn rewrite_instructions(text: &str, inputs: &Inputs) -> McpRewrite {
    let evidence = Evidence::new(inputs);
    // (heading, id, text) per section; everything before the first is the preamble.
    let mut preamble = String::new();
    let mut sections: Vec<(String, String, String)> = Vec::new();
    for line in text.split_inclusive('\n') {
        match heading(line) {
            Some(h) => sections.push((h.to_string(), server_id(h), line.to_string())),
            None => match sections.last_mut() {
                Some((_, _, body)) => body.push_str(line),
                None => preamble.push_str(line),
            },
        }
    }
    let mut out = McpRewrite {
        text: preamble,
        ..Default::default()
    };
    let mut names = Vec::new();
    for (name, id, body) in sections {
        if evidence.keeps(&id) {
            out.text.push_str(&body);
            out.described.push(id);
        } else {
            out.deferred.insert(id.clone(), body);
            out.collapsed.push(id);
            names.push(name);
        }
    }
    if out.collapsed.is_empty() {
        return McpRewrite {
            text: text.to_string(),
            described: out.described,
            ..Default::default()
        };
    }
    if !out.text.ends_with('\n') {
        out.text.push('\n');
    }
    out.text.push_str(OTHER_SERVERS);
    out.text.push_str(&names.join(", "));
    out.text.push('\n');
    out
}

/// Rewrite the `deferred_tools_delta` text.
pub fn rewrite_tools(text: &str, inputs: &Inputs) -> McpRewrite {
    let evidence = Evidence::new(inputs);
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for line in text.lines() {
        if let Some(id) = tool_server(line.trim()) {
            *counts.entry(id).or_default() += 1;
        }
    }
    let mut out = McpRewrite::default();
    let mut emitted = HashSet::new();
    for line in text.split_inclusive('\n') {
        let Some(id) = tool_server(line.trim()) else {
            out.text.push_str(line);
            continue;
        };
        if evidence.keeps(id) {
            if emitted.insert(id) {
                out.described.push(id.to_string());
            }
            out.text.push_str(line);
        } else if emitted.insert(id) {
            out.collapsed.push(id.to_string());
            out.text.push_str(&format!(
                "mcp__{id}__* ({} tools; ToolSearch finds them)\n",
                counts[id]
            ));
        }
    }
    if out.collapsed.is_empty() {
        return McpRewrite {
            text: text.to_string(),
            described: out.described,
            ..Default::default()
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{Record, Turn};
    use proptest::prelude::*;
    use std::path::Path;

    const NOW: u64 = 1_800_000_000;
    const DAY: u64 = 24 * 3600;

    const INSTRUCTIONS: &str = "\
# MCP Server Instructions

The following MCP servers have provided instructions:

## claude.ai Atlassian MCP
Atlassian MCP server. Three layers of tools:
# Jira custom fields (story points, etc.)
Use getJiraIssue.

## claude.ai Figma
Use get_design_context.

## pencil
Pencil edits .pen files.
";

    const TOOLS: &str = "\
The following deferred tools are now available via ToolSearch:
CronCreate
mcp__claude_ai_Figma__get_design_context
mcp__claude_ai_Figma__use_figma
Monitor
mcp__pencil__batch_get
mcp__claude_ai_Figma__whoami
mcp__claude_ai_Atlassian_MCP__search

If the user's request might be served by one of these servers, call ToolSearch.
";

    fn rec(ts: u64, event: Event) -> Record {
        Record { v: 1, ts, event }
    }

    fn turn(ts: u64, project: &str, mcp: &[&str]) -> Record {
        rec(
            ts,
            Event::Turn(Turn {
                project: project.into(),
                model: "m".into(),
                input: 0,
                output: 0,
                cache_read: 0,
                cache_write: 0,
                agent: false,
                aborted: false,
                lens_before: None,
                lens_after: None,
                requests: None,
                mcp: mcp.iter().map(|s| s.to_string()).collect(),
                breaks: vec![],
            }),
        )
    }

    fn seen(ts: u64, names: &[&str]) -> Record {
        rec(
            ts,
            Event::Seen {
                names: names.iter().map(|s| s.to_string()).collect(),
            },
        )
    }

    fn baseline() -> Record {
        seen(
            NOW - 60 * DAY,
            &[
                "mcp:claude_ai_Atlassian_MCP",
                "mcp:claude_ai_Figma",
                "mcp:pencil",
            ],
        )
    }

    fn inputs<'a>(records: &'a [Record], keep: &'a [String]) -> Inputs<'a> {
        Inputs {
            records,
            project: "/work/app",
            project_dir: Path::new("/nonexistent"),
            keep,
            now: NOW,
        }
    }

    #[test]
    fn ids_follow_the_tool_prefix_convention() {
        assert_eq!(server_id("claude.ai Figma"), "claude_ai_Figma");
        assert_eq!(
            server_id("claude.ai Atlassian MCP"),
            "claude_ai_Atlassian_MCP"
        );
        assert_eq!(server_id("pencil"), "pencil");
        assert_eq!(server_id("  odd -- name! "), "odd_name");
        assert_eq!(
            tool_server("mcp__claude_ai_Figma__use_figma"),
            Some("claude_ai_Figma")
        );
        assert_eq!(tool_server("mcp__pencil__batch_get"), Some("pencil"));
        assert_eq!(tool_server("mcp__nounderscore"), None);
        // An empty server id is no server.
        assert_eq!(tool_server("mcp____tool"), None);
        assert_eq!(tool_server("Monitor"), None);
        assert_eq!(
            server_ids(INSTRUCTIONS),
            ["claude_ai_Atlassian_MCP", "claude_ai_Figma", "pencil"]
        );
        assert_eq!(
            server_ids(TOOLS),
            ["claude_ai_Figma", "pencil", "claude_ai_Atlassian_MCP"]
        );
    }

    #[test]
    fn instructions_keep_used_servers_and_defer_the_rest() {
        let records = vec![baseline(), turn(NOW - DAY, "/work/app", &["pencil"])];
        let out = rewrite_instructions(INSTRUCTIONS, &inputs(&records, &[]));
        assert_eq!(out.described, ["pencil"]);
        assert_eq!(
            out.collapsed,
            ["claude_ai_Atlassian_MCP", "claude_ai_Figma"]
        );
        assert_eq!(
            out.text,
            format!(
                "# MCP Server Instructions\n\nThe following MCP servers have provided instructions:\n\n\
                 ## pencil\nPencil edits .pen files.\n\
                 {OTHER_SERVERS}claude.ai Atlassian MCP, claude.ai Figma\n"
            )
        );
        // The Atlassian section keeps its `#` subheadings.
        assert_eq!(
            out.deferred["claude_ai_Atlassian_MCP"],
            "## claude.ai Atlassian MCP\nAtlassian MCP server. Three layers of tools:\n# Jira custom fields (story points, etc.)\nUse getJiraIssue.\n\n"
        );
        assert_eq!(
            out.deferred["claude_ai_Figma"],
            "## claude.ai Figma\nUse get_design_context.\n\n"
        );
    }

    #[test]
    fn tools_collapse_per_server_at_its_first_line() {
        let records = vec![baseline(), turn(NOW - DAY, "/work/app", &["pencil"])];
        let out = rewrite_tools(TOOLS, &inputs(&records, &[]));
        assert_eq!(
            out.text,
            "The following deferred tools are now available via ToolSearch:\n\
             CronCreate\n\
             mcp__claude_ai_Figma__* (3 tools; ToolSearch finds them)\n\
             Monitor\n\
             mcp__pencil__batch_get\n\
             mcp__claude_ai_Atlassian_MCP__* (1 tools; ToolSearch finds them)\n\
             \n\
             If the user's request might be served by one of these servers, call ToolSearch.\n"
        );
        assert!(out.deferred.is_empty());
    }

    #[test]
    fn keep_rules_match_skills_lens() {
        // Used elsewhere recently, pinned, and new since the baseline: all kept.
        let records = vec![
            baseline(),
            turn(NOW - 10 * DAY, "/elsewhere", &["claude_ai_Figma"]),
            seen(NOW - 2 * DAY, &["mcp:brand_new"]),
        ];
        let keep = vec!["pencil".to_string()];
        let text = format!("{INSTRUCTIONS}\n## brand new\nNew.\n");
        let out = rewrite_instructions(&text, &inputs(&records, &keep));
        assert_eq!(out.described, ["claude_ai_Figma", "pencil", "brand_new"]);
        assert_eq!(out.collapsed, ["claude_ai_Atlassian_MCP"]);
        // Old uses elsewhere don't count; nothing collapsed means unchanged text.
        let records = vec![
            baseline(),
            turn(NOW - 40 * DAY, "/elsewhere", &["claude_ai_Figma"]),
        ];
        let out = rewrite_tools(TOOLS, &inputs(&records, &[]));
        assert_eq!(out.collapsed.len(), 3);
        let everything: Vec<String> = ["claude_ai_Atlassian_MCP", "claude_ai_Figma", "pencil"]
            .map(String::from)
            .to_vec();
        let out = rewrite_instructions(INSTRUCTIONS, &inputs(&[baseline()], &everything));
        assert_eq!(out.text, INSTRUCTIONS);
        assert!(out.collapsed.is_empty() && out.deferred.is_empty());
        // Unchanged text still reports what it described.
        assert_eq!(out.described, everything);
        let out = rewrite_tools(TOOLS, &inputs(&[baseline()], &everything));
        assert_eq!(out.text, TOOLS);
        assert_eq!(
            out.described,
            ["claude_ai_Figma", "pencil", "claude_ai_Atlassian_MCP"]
        );
    }

    #[test]
    fn first_run_is_the_server_baseline() {
        // No server seen yet: nothing is new, unused servers collapse at once.
        let out = rewrite_instructions(INSTRUCTIONS, &inputs(&[], &[]));
        assert_eq!(out.collapsed.len(), 3);
        // A skills-only seen history doesn't make servers new either.
        let records = vec![seen(NOW - DAY, &["some-skill"])];
        let out = rewrite_instructions(INSTRUCTIONS, &inputs(&records, &[]));
        assert_eq!(out.collapsed.len(), 3);
    }

    #[test]
    fn servers_first_seen_within_a_day_of_the_baseline_are_baseline_too() {
        // Instructions and the tools list arrive as separate texts: a server
        // that only has tools is seen moments after the baseline was set.
        let records = vec![
            seen(NOW - 60 * DAY, &["mcp:claude_ai_Figma", "mcp:pencil"]),
            seen(NOW - 60 * DAY + 60, &["mcp:claude_ai_Atlassian_MCP"]),
        ];
        let out = rewrite_tools(TOOLS, &inputs(&records, &[]));
        assert_eq!(out.collapsed.len(), 3, "{out:?}");
        // On the very first day, an unseen server is baseline as well.
        let records = vec![seen(NOW - 60, &["mcp:claude_ai_Figma"])];
        let out = rewrite_tools(TOOLS, &inputs(&records, &[]));
        assert_eq!(out.collapsed.len(), 3, "{out:?}");
    }

    #[test]
    fn text_without_servers_passes_through() {
        for text in [
            "",
            "no servers here\n",
            "# MCP Server Instructions\n\nnone\n",
        ] {
            let out = rewrite_instructions(text, &inputs(&[], &[]));
            assert_eq!(out.text, text);
            let out = rewrite_tools(text, &inputs(&[], &[]));
            assert_eq!(out.text, text);
        }
    }

    proptest! {
        #[test]
        fn no_server_id_is_ever_lost(
            servers in prop::collection::vec("[a-z]{1,5}( [a-z]{1,5})?", 0..6),
            used in any::<u8>(),
        ) {
            let mut instructions = String::from("# MCP\n\n");
            let mut tools = String::from("Deferred:\nCronCreate\n");
            for (i, s) in servers.iter().enumerate() {
                instructions.push_str(&format!("## {s}\nbody {i}\n\n"));
                for t in 0..(i % 3 + 1) {
                    tools.push_str(&format!("mcp__{}__t{t}\n", server_id(s)));
                }
            }
            let ids: Vec<String> = servers.iter().map(|s| server_id(s)).collect();
            let used_ids: Vec<&str> = ids.iter().enumerate().filter(|(i, _)| used & (1 << (i % 8)) != 0).map(|(_, s)| s.as_str()).collect();
            let records = vec![turn(NOW - DAY, "/work/app", &used_ids)];
            let a = rewrite_instructions(&instructions, &inputs(&records, &[]));
            let b = rewrite_tools(&tools, &inputs(&records, &[]));
            for id in &ids {
                prop_assert!(a.text.contains(id.as_str()) || a.text.contains(&servers[ids.iter().position(|x| x == id).unwrap()]), "{id} lost from {}", a.text);
                prop_assert!(b.text.contains(&format!("mcp__{id}__")), "{id} lost from {}", b.text);
            }
            prop_assert!(a.text.len() <= instructions.len() + 200);
            prop_assert!(b.text.contains("CronCreate"));
        }
    }
}
