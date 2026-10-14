//! Girth: just-in-time, project-scoped agent skills.

pub mod config;
pub mod frontmatter;
pub mod ledger;
pub mod measure;
pub mod record;
pub mod search;
pub mod sections;
pub mod skill;
pub mod stats;
#[cfg(test)]
pub(crate) mod testutil;
pub mod validate;
