//! The contract validator must never panic on arbitrary SKILL.md text.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = girth::validate::validate_text("girth-fuzz", &text);
});
