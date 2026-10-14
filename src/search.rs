//! Ranking skills (and the crafted-skill cache) for a need.

use std::collections::HashSet;

use serde::Serialize;

use crate::skill::{CRAFTED_PREFIX, SkillInfo};

const STOPWORDS: [&str; 34] = [
    "a", "an", "and", "are", "as", "at", "be", "by", "do", "for", "from", "how", "in", "is", "it",
    "me", "my", "need", "of", "on", "or", "the", "to", "use", "using", "want", "we", "what",
    "when", "with", "i", "you", "our", "this",
];
pub const NAME_WEIGHT: u32 = 5;
pub const DESCRIPTION_WEIGHT: u32 = 2;
pub const HEADING_WEIGHT: u32 = 1;
pub const MAX_MATCHED_HEADINGS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub skill: SkillInfo,
    pub score: u32,
    pub exact: bool,
    pub matched_headings: Vec<(String, String)>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct HeadingRef {
    pub file: String,
    pub heading: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct HitJson {
    pub name: String,
    pub path: String,
    pub score: u32,
    pub exact: bool,
    pub crafted: bool,
    pub description: String,
    pub skill_md_bytes: u64,
    pub total_md_bytes: u64,
    pub matched_headings: Vec<HeadingRef>,
}

/// Lowercase alphanumeric runs longer than one char, minus stopwords.
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() > 1 && !STOPWORDS.contains(w))
        .map(str::to_string)
        .collect()
}

fn token_set(text: &str) -> HashSet<String> {
    tokenize(text).into_iter().collect()
}

/// Per distinct term: name match 5, description 2, any heading 1.
pub fn score(skill: &SkillInfo, terms: &[String]) -> u32 {
    let name = token_set(&skill.name);
    let description = token_set(&skill.description);
    let headings: HashSet<String> = skill
        .headings
        .iter()
        .flat_map(|(_, h)| tokenize(h))
        .collect();
    let distinct: HashSet<&String> = terms.iter().collect();
    distinct
        .into_iter()
        .map(|term| {
            NAME_WEIGHT * u32::from(name.contains(term))
                + DESCRIPTION_WEIGHT * u32::from(description.contains(term))
                + HEADING_WEIGHT * u32::from(headings.contains(term))
        })
        .sum()
}

fn is_exact(skill: &SkillInfo, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let joined = terms.join("-");
    let name = skill.name.to_lowercase();
    joined == name || name.strip_prefix(CRAFTED_PREFIX) == Some(joined.as_str())
}

/// Headings containing any informative term, most terms first, then document order.
///
/// Terms already in the skill's name match nearly every heading of that skill, so
/// they are ignored unless they are the only terms.
fn matched_headings(skill: &SkillInfo, terms: &HashSet<String>) -> Vec<(String, String)> {
    let name = token_set(&skill.name);
    let informative: HashSet<String> = terms.difference(&name).cloned().collect();
    let terms = if informative.is_empty() {
        terms
    } else {
        &informative
    };
    let mut scored: Vec<(usize, usize, &(String, String))> = skill
        .headings
        .iter()
        .enumerate()
        .map(|(index, heading)| {
            (
                token_set(&heading.1).intersection(terms).count(),
                index,
                heading,
            )
        })
        .filter(|(overlap, _, _)| *overlap > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(MAX_MATCHED_HEADINGS)
        .map(|(_, _, h)| h.clone())
        .collect()
}

/// Drop zero scores; exact matches first, then score desc, name, path.
pub fn rank(skills: &[SkillInfo], query: &str, limit: usize) -> Vec<SearchHit> {
    let terms = tokenize(query);
    let term_set: HashSet<String> = terms.iter().cloned().collect();
    let mut hits: Vec<SearchHit> = skills
        .iter()
        .filter_map(|skill| {
            let score = score(skill, &terms);
            (score > 0).then(|| SearchHit {
                skill: skill.clone(),
                score,
                exact: is_exact(skill, &terms),
                matched_headings: matched_headings(skill, &term_set),
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        (
            !a.exact,
            std::cmp::Reverse(a.score),
            &a.skill.name,
            &a.skill.path,
        )
            .cmp(&(
                !b.exact,
                std::cmp::Reverse(b.score),
                &b.skill.name,
                &b.skill.path,
            ))
    });
    hits.truncate(limit);
    hits
}

impl From<&SearchHit> for HitJson {
    fn from(hit: &SearchHit) -> Self {
        let s = &hit.skill;
        Self {
            name: s.name.clone(),
            path: s.path.display().to_string(),
            score: hit.score,
            exact: hit.exact,
            crafted: s.crafted(),
            description: s.description.clone(),
            skill_md_bytes: s.skill_md_bytes,
            total_md_bytes: s.total_md_bytes,
            matched_headings: hit
                .matched_headings
                .iter()
                .map(|(file, heading)| HeadingRef {
                    file: file.clone(),
                    heading: heading.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    fn mk(name: &str, description: &str, headings: &[&str]) -> SkillInfo {
        SkillInfo {
            name: name.into(),
            description: description.into(),
            path: PathBuf::from("/skills").join(name),
            skill_md_bytes: 10,
            total_md_bytes: 10,
            headings: headings
                .iter()
                .map(|h| ("SKILL.md".to_string(), h.to_string()))
                .collect(),
            fell_back: false,
        }
    }

    fn names(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.skill.name.as_str()).collect()
    }

    fn terms(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn tokenize_lowercases_splits_and_drops_stopwords_and_short_words() {
        assert_eq!(
            tokenize("How to do Caption-Timing in 3D a"),
            ["caption", "timing", "3d"]
        );
    }

    #[test]
    fn tokenize_keeps_two_char_words_but_not_one_char() {
        assert_eq!(tokenize("x y zz"), ["zz"]);
    }

    #[test]
    fn score_weights_name_description_heading_once_per_distinct_term() {
        let s = mk("video-captions", "Timing for captions", &["Caption timing"]);
        assert_eq!(score(&s, &terms(&["captions"])), 5 + 2);
        assert_eq!(score(&s, &terms(&["timing"])), 2 + 1);
        assert_eq!(score(&s, &terms(&["caption"])), 1);
        assert_eq!(score(&s, &terms(&["captions", "captions"])), 7);
        assert_eq!(score(&s, &[]), 0);
    }

    #[test]
    fn rank_drops_zero_scores_and_orders_by_score_then_name() {
        let skills = [
            mk("b-video", "captions", &[]),
            mk("a-video", "captions", &[]),
            mk("captions-pro", "", &[]),
            mk("x", "", &[]),
        ];
        assert_eq!(
            names(&rank(&skills, "captions", 10)),
            ["captions-pro", "a-video", "b-video"]
        );
    }

    #[test]
    fn exact_name_ranks_first_even_with_lower_score() {
        let hits = rank(
            &[
                mk("video-captions", "captions", &["captions"]),
                mk("captions", "", &[]),
            ],
            "captions",
            10,
        );
        assert_eq!(names(&hits), ["captions", "video-captions"]);
        assert!(hits[0].exact && !hits[1].exact);
    }

    #[test]
    fn crafted_skill_for_domain_counts_as_exact() {
        let hits = rank(
            &[
                mk("video-captions", "captions", &[]),
                mk("girth-captions", "", &[]),
            ],
            "captions",
            10,
        );
        assert_eq!(hits[0].skill.name, "girth-captions");
        assert!(hits[0].exact);
    }

    #[test]
    fn matched_headings_capped_at_five_and_carry_file() {
        let headings: Vec<String> = (0..8)
            .map(|i| format!("Caption part {i}"))
            .chain(["Color grading".to_string()])
            .collect();
        let refs: Vec<&str> = headings.iter().map(String::as_str).collect();
        let hit = &rank(&[mk("v", "", &refs)], "caption", 10)[0];
        assert_eq!(hit.matched_headings.len(), 5);
        assert_eq!(
            hit.matched_headings[0],
            ("SKILL.md".to_string(), "Caption part 0".to_string())
        );
    }

    #[test]
    fn matched_headings_prefer_headings_with_more_query_terms() {
        let s = mk(
            "v",
            "",
            &[
                "Hyperframes intro",
                "Color",
                "Caption timing in hyperframes",
            ],
        );
        let hit = &rank(&[s], "caption hyperframes", 10)[0];
        let got: Vec<&str> = hit
            .matched_headings
            .iter()
            .map(|(_, h)| h.as_str())
            .collect();
        assert_eq!(got, ["Caption timing in hyperframes", "Hyperframes intro"]);
    }

    #[test]
    fn matched_headings_ignore_terms_already_in_the_skill_name() {
        let s = mk(
            "hyperframes-animation",
            "",
            &[
                "HyperFrames Animation",
                "Anime.js for HyperFrames",
                "Per-Word Kinetic Typography",
            ],
        );
        let hit = &rank(&[s], "word caption hyperframes", 10)[0];
        let got: Vec<&str> = hit
            .matched_headings
            .iter()
            .map(|(_, h)| h.as_str())
            .collect();
        assert_eq!(got, ["Per-Word Kinetic Typography"]);
    }

    #[test]
    fn matched_headings_use_all_terms_when_every_term_is_in_the_name() {
        let s = mk("captions", "", &["Captions overview", "Color"]);
        let hit = &rank(&[s], "captions", 10)[0];
        assert_eq!(
            hit.matched_headings,
            [("SKILL.md".to_string(), "Captions overview".to_string())]
        );
    }

    #[test]
    fn rank_limit() {
        let skills: Vec<SkillInfo> = (0..5)
            .map(|i| mk(&format!("s{i}"), "captions", &[]))
            .collect();
        assert_eq!(rank(&skills, "captions", 2).len(), 2);
    }

    #[test]
    fn hit_json_shape_is_frozen() {
        let hit = &rank(
            &[mk("girth-captions", "captions", &["Caption timing"])],
            "caption",
            10,
        )[0];
        let json = serde_json::to_value(HitJson::from(hit)).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "crafted",
                "description",
                "exact",
                "matched_headings",
                "name",
                "path",
                "score",
                "skill_md_bytes",
                "total_md_bytes"
            ]
        );
        assert_eq!(
            json["matched_headings"][0],
            serde_json::json!({"file": "SKILL.md", "heading": "Caption timing"})
        );
        assert_eq!(json["crafted"], true);
    }

    fn skill_strategy() -> impl Strategy<Value = SkillInfo> {
        (
            "[a-z]{1,6}(-[a-z]{1,6}){0,2}",
            "[abc def ghi]{0,30}",
            proptest::collection::vec("[abc def ghi]{0,20}", 0..3),
        )
            .prop_map(|(name, description, headings)| {
                let refs: Vec<&str> = headings.iter().map(String::as_str).collect();
                mk(&name, &description, &refs)
            })
    }

    proptest! {
        #[test]
        fn rank_is_order_independent_and_sorted(
            skills in proptest::collection::vec(skill_strategy(), 0..8),
            query in "[abc def-]{0,12}",
        ) {
            let mut unique = skills;
            unique.sort_by(|a, b| a.name.cmp(&b.name));
            unique.dedup_by(|a, b| a.name == b.name);
            let hits = rank(&unique, &query, 100);
            let reversed: Vec<SkillInfo> = unique.iter().rev().cloned().collect();
            prop_assert_eq!(&hits, &rank(&reversed, &query, 100));
            let keys: Vec<(bool, std::cmp::Reverse<u32>)> = hits.iter().map(|h| (!h.exact, std::cmp::Reverse(h.score))).collect();
            let mut sorted = keys.clone();
            sorted.sort();
            prop_assert_eq!(keys, sorted);
        }
    }
}
