//! Untrusted SKILL.md text must never panic the frontmatter parsers.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let (frontmatter, _body) = girth::frontmatter::split_frontmatter(&text);
    let _ = girth::frontmatter::parse_strict(&frontmatter);
    let _ = girth::frontmatter::parse_lenient(&frontmatter);
});
