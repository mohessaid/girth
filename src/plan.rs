//! Plan windows (Claude's 5-hour and 7-day limits): the mod records Claude Code's
//! rate-limit readings, and this says how full the fullest window is, when it resets,
//! and, from the window's own recent readings, whether it runs out first.

use crate::ledger::{Event, PlanReading, Record};

/// A window this full (per mille) or more gets a line.
pub const WARN_PERMILLE: u32 = 800;
/// Only readings this recent describe the window now.
const FRESH_S: u64 = 6 * 3600;
/// The pace comes from at most this far back, and needs at least `MIN_SPAN_S` of it.
const PACE_S: u64 = 24 * 3600;
const MIN_SPAN_S: u64 = 3600;

fn name(kind: &str) -> String {
    match kind {
        "five_hour" => "5-hour plan window".into(),
        "seven_day" => "7-day plan window".into(),
        "spend_limit" => "spend limit".into(),
        other => format!("{other} window"),
    }
}

/// `2d 5h`, `3h 15m`, `40 min`.
pub fn span(s: u64) -> String {
    let m = (s + 30) / 60;
    match m {
        0..=59 => format!("{} min", m.max(1)),
        60..=1439 => format!("{}h {}m", m / 60, m % 60),
        _ => format!("{}d {}h", m / 1440, m % 1440 / 60),
    }
}

fn percent(permille: u32) -> String {
    if permille % 10 == 0 {
        format!("{}%", permille / 10)
    } else {
        format!("{}.{}%", permille / 10, permille % 10)
    }
}

/// The line for the fullest window at `WARN_PERMILLE` or more, read in the last
/// `FRESH_S`; `None` when no window is that full.
pub fn line(records: &[Record], now: u64) -> Option<String> {
    let readings: Vec<(u64, &PlanReading)> = records
        .iter()
        .filter_map(|r| match &r.event {
            Event::Plan(p) => Some((r.ts, p)),
            _ => None,
        })
        .collect();
    let mut latest: Vec<(u64, &PlanReading)> = Vec::new();
    for (ts, p) in &readings {
        if now.saturating_sub(*ts) > FRESH_S {
            continue;
        }
        match latest.iter_mut().find(|(_, q)| q.window == p.window) {
            Some(slot) if slot.0 <= *ts => *slot = (*ts, p),
            Some(_) => {}
            None => latest.push((*ts, p)),
        }
    }
    let (ts, top) = latest
        .into_iter()
        .filter(|(_, p)| p.permille >= WARN_PERMILLE)
        .max_by_key(|(ts, p)| (p.permille, *ts))?;
    let mut text = format!(
        "{} of your {} is used",
        percent(top.permille),
        name(&top.window)
    );
    let resets = top.resets_at.filter(|r| *r > now);
    // The same window (same reset) read earlier, within the pace span.
    let first = readings
        .iter()
        .filter(|(t, p)| {
            p.window == top.window
                && p.resets_at == top.resets_at
                && ts.saturating_sub(*t) <= PACE_S
        })
        .min_by_key(|(t, _)| *t);
    let pace = first.and_then(|(t0, p0)| {
        let spanned = ts - t0;
        let rose = top.permille.checked_sub(p0.permille).filter(|r| *r > 0)?;
        (spanned >= MIN_SPAN_S).then_some((spanned, rose))
    });
    match (pace, resets) {
        (Some((spanned, rose)), Some(r)) => {
            let left = 1000u64.saturating_sub(top.permille.into());
            let out_at = ts + left * spanned / u64::from(rose);
            if out_at < r {
                text += &format!(
                    "; at the last {}'s pace it runs out in {}, {} before it resets",
                    span(spanned),
                    span(out_at.saturating_sub(now)),
                    span(r - out_at)
                );
            } else {
                text += &format!(
                    "; at the last {}'s pace it lasts until it resets in {}",
                    span(spanned),
                    span(r - now)
                );
            }
        }
        (_, Some(r)) => text += &format!("; it resets in {}", span(r - now)),
        _ => {}
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(ts: u64, kind: &str, permille: u32, resets_at: Option<u64>) -> Record {
        Record {
            v: 1,
            ts,
            event: Event::Plan(PlanReading {
                window: kind.into(),
                permille,
                resets_at,
            }),
        }
    }

    const NOW: u64 = 1_000_000;
    const H: u64 = 3600;

    #[test]
    fn spans_read_like_a_clock() {
        assert_eq!(span(0), "1 min");
        assert_eq!(span(40 * 60), "40 min");
        assert_eq!(span(3 * H + 15 * 60), "3h 15m");
        assert_eq!(span(2 * 24 * H + 5 * H), "2d 5h");
    }

    #[test]
    fn under_the_line_or_stale_says_nothing() {
        assert_eq!(line(&[reading(NOW, "seven_day", 799, None)], NOW), None);
        assert_eq!(
            line(&[reading(NOW - 7 * H, "seven_day", 950, None)], NOW),
            None
        );
        assert_eq!(line(&[], NOW), None);
    }

    #[test]
    fn the_fullest_fresh_window_and_its_reset() {
        let records = vec![
            reading(NOW - H, "five_hour", 900, Some(NOW + 40 * 60)),
            reading(NOW, "five_hour", 410, Some(NOW + 40 * 60)),
            reading(NOW, "seven_day", 885, Some(NOW + 2 * 24 * H + 5 * H)),
        ];
        // five_hour's latest is 41%: the 90% reading is older and doesn't count.
        assert_eq!(
            line(&records, NOW).unwrap(),
            "88.5% of your 7-day plan window is used; it resets in 2d 5h"
        );
        assert_eq!(
            line(&[reading(NOW, "spend_limit", 1010, None)], NOW).unwrap(),
            "101% of your spend limit is used"
        );
    }

    #[test]
    fn a_window_filling_faster_than_it_resets_says_when_it_runs_out() {
        let resets = NOW + 3 * 24 * H;
        // 70% → 88% over 10 hours: 1.8% an hour, so 12% more takes 6h40m.
        let records = vec![
            reading(NOW - 30 * H, "seven_day", 500, Some(resets)),
            reading(NOW - 10 * H, "seven_day", 700, Some(resets)),
            reading(NOW - 5 * H, "seven_day", 790, Some(resets)),
            reading(NOW, "seven_day", 880, Some(resets)),
        ];
        assert_eq!(
            line(&records, NOW).unwrap(),
            "88% of your 7-day plan window is used; at the last 10h 0m's pace it runs out in 6h 40m, 2d 17h before it resets"
        );
        // Slower than the reset: it lasts.
        let slow = vec![
            reading(NOW - 20 * H, "seven_day", 870, Some(NOW + 5 * H)),
            reading(NOW, "seven_day", 880, Some(NOW + 5 * H)),
        ];
        assert_eq!(
            line(&slow, NOW).unwrap(),
            "88% of your 7-day plan window is used; at the last 20h 0m's pace it lasts until it resets in 5h 0m"
        );
    }

    #[test]
    fn boundaries_freshness_order_resets_and_pace_span() {
        // Exactly FRESH_S old still describes the window.
        assert!(line(&[reading(NOW - 6 * H, "seven_day", 900, None)], NOW).is_some());
        // Each window keeps its own latest reading, whatever order the records come in.
        let two = vec![
            reading(NOW - 2 * H, "seven_day", 900, None),
            reading(NOW - H, "five_hour", 100, None),
        ];
        assert_eq!(
            line(&two, NOW).unwrap(),
            "90% of your 7-day plan window is used"
        );
        let newer_first = vec![
            reading(NOW, "seven_day", 100, None),
            reading(NOW - H, "seven_day", 900, None),
        ];
        assert_eq!(line(&newer_first, NOW), None);
        // A reset at this very moment is no reset to wait for.
        assert_eq!(
            line(&[reading(NOW, "seven_day", 880, Some(NOW))], NOW).unwrap(),
            "88% of your 7-day plan window is used"
        );
        // A reading exactly PACE_S earlier still sets the pace.
        let day = vec![
            reading(NOW - 24 * H, "seven_day", 400, Some(NOW + 10 * 24 * H)),
            reading(NOW, "seven_day", 880, Some(NOW + 10 * 24 * H)),
        ];
        assert!(
            line(&day, NOW)
                .unwrap()
                .contains("at the last 1d 0h's pace")
        );
        // Running out exactly at the reset is lasting until it.
        let even = vec![
            reading(NOW - 10 * H, "seven_day", 700, Some(NOW + 24_000)),
            reading(NOW, "seven_day", 880, Some(NOW + 24_000)),
        ];
        assert!(
            line(&even, NOW).unwrap().contains("lasts until it resets"),
            "{:?}",
            line(&even, NOW)
        );
    }

    #[test]
    fn no_pace_from_another_window_a_short_span_or_no_rise() {
        let r = Some(NOW + 2 * 24 * H);
        let base = "88% of your 7-day plan window is used; it resets in 2d 0h";
        // An earlier window (another reset time) isn't this one's pace.
        let other = vec![
            reading(NOW - 5 * H, "seven_day", 100, Some(NOW - H)),
            reading(NOW, "seven_day", 880, r),
        ];
        assert_eq!(line(&other, NOW).unwrap(), base);
        let short = vec![
            reading(NOW - 30 * 60, "seven_day", 860, r),
            reading(NOW, "seven_day", 880, r),
        ];
        assert_eq!(line(&short, NOW).unwrap(), base);
        let flat = vec![
            reading(NOW - 5 * H, "seven_day", 880, r),
            reading(NOW, "seven_day", 880, r),
        ];
        assert_eq!(line(&flat, NOW).unwrap(), base);
    }
}
