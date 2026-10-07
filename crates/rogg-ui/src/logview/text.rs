use super::model::{Span, SpanStyle};
use crate::theme::{Color, Palette};
use std::ops::Range;

/// Lines longer than this are cut for display (spec §4.4); matching still sees the full line.
pub const MAX_DISPLAY_BYTES: usize = 64 * 1024;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Segment {
    pub range: Range<usize>,
    pub fg: Color,
    pub hit_bg: Option<Color>,
    pub outline: bool,
}

/// Cuts `text` to at most `max_bytes` (on a char boundary, appending `…`) and replaces ASCII control
/// characters with spaces one-for-one, so span byte offsets stay valid.
pub fn display_text(text: &str, max_bytes: usize) -> (String, bool) {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut out: String = text[..end].chars().map(|c| if c.is_ascii_control() { ' ' } else { c }).collect();
    let truncated = end < text.len();
    if truncated {
        out.push('…');
    }
    (out, truncated)
}

/// The run of non-whitespace characters around byte `index` (snapped back to a char boundary); None on
/// whitespace, on the `…` truncation marker and for empty text. An index at the end belongs to the last word.
pub fn token_at(text: &str, index: usize) -> Option<String> {
    let index = text.floor_char_boundary(index.min(text.len()));
    let is_word = |c: char| !c.is_whitespace() && c != '…';
    // The character under the index must itself be a word character (the end of the text excepted).
    if text[index..].chars().next().is_some_and(|c| !is_word(c)) {
        return None;
    }
    let start = text[..index].char_indices().rev().take_while(|&(_, c)| is_word(c)).last().map_or(index, |(i, _)| i);
    let end = text[index..].char_indices().find(|&(_, c)| !is_word(c)).map_or(text.len(), |(i, _)| index + i);
    (start < end).then(|| text[start..end].to_string())
}

fn resolve(style: SpanStyle, p: &Palette) -> Option<Color> {
    match style {
        SpanStyle::Faint => Some(p.faint),
        SpanStyle::Muted => Some(p.muted),
        SpanStyle::Error => Some(p.error),
        SpanStyle::Amber => Some(p.amber),
        SpanStyle::Fg(c) => Some(c),
        SpanStyle::Hit(_) | SpanStyle::SearchHit => None,
    }
}

/// Splits `0..len` into maximal runs of identical style. Later foreground spans override earlier ones,
/// a filter hit overrides foreground (first hit wins), and search hits add an outline.
///
/// O(n log n) event-based sweep algorithm: creates start/end events for each span,
/// processes in position order while tracking active spans. Each span is added/removed once.
pub fn segments(len: usize, spans: &[Span], base: Color, p: &Palette) -> Vec<Segment> {
    if len == 0 {
        return Vec::new();
    }

    use std::collections::BTreeSet;

    // Create events for span boundaries: (position, is_start, span_index)
    let mut events = Vec::new();
    for (idx, span) in spans.iter().enumerate() {
        let start = span.range.start.min(len);
        let end = span.range.end.min(len);
        if start < end {
            events.push((start, true, idx));   // Start event
            events.push((end, false, idx));    // End event
        }
    }

    // Sort by position (starts before ends at the same position). Order within a position does not
    // matter: every event there is applied before the next segment is emitted.
    events.sort_by_key(|e| (e.0, !e.1));

    // Track active spans by type
    let mut active_fg: BTreeSet<usize> = BTreeSet::new();
    let mut active_hits: BTreeSet<usize> = BTreeSet::new();
    let mut active_search_hits: usize = 0;

    let mut out: Vec<Segment> = Vec::new();
    let mut last_pos = 0;

    let mut event_idx = 0;
    while event_idx < events.len() {
        let pos = events[event_idx].0;

        // Emit segment for [last_pos, pos) with current active style
        if last_pos < pos {
            // Compute style from active spans
            let fg = if let Some(&idx) = active_fg.iter().next_back() {
                resolve(spans[idx].style, p).unwrap_or(base)
            } else {
                base
            };

            let (hit_fg, hit_bg) = if let Some(&idx) = active_hits.iter().next() {
                if let SpanStyle::Hit(slot) = spans[idx].style {
                    let h = p.hits[slot % p.hits.len()];
                    (h.fg, h.bg)
                } else {
                    (fg, base)
                }
            } else {
                (fg, base)
            };

            let final_fg = if active_hits.is_empty() { fg } else { hit_fg };
            let hit_bg_opt = if active_hits.is_empty() { None } else { Some(hit_bg) };
            let outline = active_search_hits > 0;

            if let Some(last) = out.last_mut() {
                if last.range.end == last_pos && last.fg == final_fg && last.hit_bg == hit_bg_opt && last.outline == outline {
                    last.range.end = pos;
                } else {
                    out.push(Segment { range: last_pos..pos, fg: final_fg, hit_bg: hit_bg_opt, outline });
                }
            } else {
                out.push(Segment { range: last_pos..pos, fg: final_fg, hit_bg: hit_bg_opt, outline });
            }
        }

        // Process all events at this position
        while event_idx < events.len() && events[event_idx].0 == pos {
            let (_, is_start, idx) = events[event_idx];
            let style = spans[idx].style;

            if is_start {
                match style {
                    SpanStyle::Faint | SpanStyle::Muted | SpanStyle::Error | SpanStyle::Amber | SpanStyle::Fg(_) => {
                        active_fg.insert(idx);
                    }
                    SpanStyle::Hit(_) => {
                        active_hits.insert(idx);
                    }
                    SpanStyle::SearchHit => {
                        active_search_hits += 1;
                    }
                }
            } else {
                match style {
                    SpanStyle::Faint | SpanStyle::Muted | SpanStyle::Error | SpanStyle::Amber | SpanStyle::Fg(_) => {
                        active_fg.remove(&idx);
                    }
                    SpanStyle::Hit(_) => {
                        active_hits.remove(&idx);
                    }
                    SpanStyle::SearchHit => {
                        active_search_hits = active_search_hits.saturating_sub(1);
                    }
                }
            }

            event_idx += 1;
        }

        last_pos = pos;
    }

    // Emit final segment [last_pos, len)
    if last_pos < len {
        let fg = if let Some(&idx) = active_fg.iter().next_back() {
            resolve(spans[idx].style, p).unwrap_or(base)
        } else {
            base
        };

        let (hit_fg, hit_bg) = if let Some(&idx) = active_hits.iter().next() {
            if let SpanStyle::Hit(slot) = spans[idx].style {
                let h = p.hits[slot % p.hits.len()];
                (h.fg, h.bg)
            } else {
                (fg, base)
            }
        } else {
            (fg, base)
        };

        let final_fg = if active_hits.is_empty() { fg } else { hit_fg };
        let hit_bg_opt = if active_hits.is_empty() { None } else { Some(hit_bg) };
        let outline = active_search_hits > 0;

        if let Some(last) = out.last_mut() {
            if last.range.end == last_pos && last.fg == final_fg && last.hit_bg == hit_bg_opt && last.outline == outline {
                last.range.end = len;
            } else {
                out.push(Segment { range: last_pos..len, fg: final_fg, hit_bg: hit_bg_opt, outline });
            }
        } else {
            out.push(Segment { range: last_pos..len, fg: final_fg, hit_bg: hit_bg_opt, outline });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Palette {
        Palette::dark()
    }

    #[test]
    fn display_text_keeps_offsets_and_replaces_controls() {
        let (s, cut) = display_text("a\tb\rc\u{1}d", 100);
        assert_eq!(s, "a b c d");
        assert!(!cut);
    }

    #[test]
    fn display_text_truncates_on_char_boundary_and_marks_it() {
        let text = format!("{}é", "x".repeat(9)); // 'é' is 2 bytes at offsets 9..11
        let (s, cut) = display_text(&text, 10);
        assert!(cut);
        assert_eq!(s, format!("{}…", "x".repeat(9)));
        let huge = "y".repeat(200 * 1024);
        let (s, cut) = display_text(&huge, MAX_DISPLAY_BYTES);
        assert!(cut);
        assert_eq!(s.len(), MAX_DISPLAY_BYTES + '…'.len_utf8());
    }

    #[test]
    fn plain_line_is_one_base_segment() {
        assert_eq!(segments(5, &[], p().log_text, &p()), vec![Segment { range: 0..5, fg: p().log_text, hit_bg: None, outline: false }]);
        assert!(segments(0, &[], p().log_text, &p()).is_empty());
    }

    #[test]
    fn faint_stamp_then_hit() {
        let spans = vec![Span { range: 0..3, style: SpanStyle::Faint }, Span { range: 5..8, style: SpanStyle::Hit(1) }];
        let segs = segments(10, &spans, p().log_text, &p());
        assert_eq!(segs.iter().map(|s| s.range.clone()).collect::<Vec<_>>(), vec![0..3, 3..5, 5..8, 8..10]);
        assert_eq!(segs[0].fg, p().faint);
        assert_eq!(segs[2].fg, p().hits[1].fg);
        assert_eq!(segs[2].hit_bg, Some(p().hits[1].bg));
        assert_eq!(segs[3].fg, p().log_text);
    }

    #[test]
    fn hit_beats_fg_and_search_adds_outline() {
        let spans = vec![
            Span { range: 0..10, style: SpanStyle::Error },
            Span { range: 2..4, style: SpanStyle::Hit(0) },
            Span { range: 3..6, style: SpanStyle::SearchHit },
        ];
        let segs = segments(10, &spans, p().log_text, &p());
        let at = |i: usize| segs.iter().find(|s| s.range.contains(&i)).unwrap().clone();
        assert_eq!(at(0).fg, p().error);
        assert_eq!(at(2).fg, p().hits[0].fg);
        assert!(!at(2).outline);
        assert!(at(3).outline && at(3).hit_bg.is_some());
        assert!(at(5).outline && at(5).fg == p().error);
    }

    #[test]
    fn semantic_styles_follow_the_palette() {
        let spans = vec![Span { range: 0..3, style: SpanStyle::Faint }, Span { range: 3..6, style: SpanStyle::Hit(0) }];
        let light = Palette::light();
        let segs = segments(6, &spans, light.log_text, &light);
        assert_eq!(segs[0].fg, light.faint);
        assert_eq!(segs[1].fg, light.hits[0].fg);
        assert_eq!(segs[1].hit_bg, Some(light.hits[0].bg));
        assert_ne!(segs[0].fg, Palette::dark().faint);
    }

    #[test]
    fn spans_past_the_end_are_clipped_and_slots_wrap() {
        let spans = vec![Span { range: 4..99, style: SpanStyle::Hit(6) }];
        let segs = segments(6, &spans, p().log_text, &p());
        assert_eq!(segs.last().unwrap().range, 4..6);
        assert_eq!(segs.last().unwrap().hit_bg, Some(p().hits[2].bg));
        let total: usize = segs.iter().map(|s| s.range.len()).sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn many_spans_hostile_line() {
        // 64 KB line with ~16,000 non-overlapping Hit spans (each 4 bytes)
        // Tests that O(n log n) algorithm handles hostile input efficiently
        let mut spans = Vec::new();
        const SPAN_SIZE: usize = 4;
        const LINE_LEN: usize = 64 * 1024;
        for i in 0..LINE_LEN / SPAN_SIZE {
            let start = i * SPAN_SIZE;
            let end = (i + 1) * SPAN_SIZE;
            spans.push(Span { range: start..end, style: SpanStyle::Hit(i % 4) });
        }

        let segs = segments(LINE_LEN, &spans, p().log_text, &p());

        // Verify full coverage (no gaps)
        let total: usize = segs.iter().map(|s| s.range.len()).sum();
        assert_eq!(total, LINE_LEN);

        // Verify every segment has a Hit background
        for seg in &segs {
            assert!(seg.hit_bg.is_some(), "segment {:?} missing hit_bg", seg.range);
        }

        // Verify runs alternate correctly at sampled positions
        for i in 0..10 {
            let pos = i * (LINE_LEN / 10);
            if pos < LINE_LEN {
                let seg = segs.iter().find(|s| s.range.contains(&pos)).expect(&format!("no segment at {}", pos));
                assert!(seg.hit_bg.is_some());
            }
        }
    }

    #[test]
    fn later_fg_overrides_earlier_fg() {
        // When two Fg spans overlap, the one with higher span index wins
        let spans = vec![
            Span { range: 0..10, style: SpanStyle::Faint },
            Span { range: 5..15, style: SpanStyle::Muted },
        ];
        let segs = segments(15, &spans, p().log_text, &p());

        // [0, 5): only Faint
        assert_eq!(segs[0].range, 0..5);
        assert_eq!(segs[0].fg, p().faint);

        // [5, 15): Muted wins (higher span index) and merges
        assert_eq!(segs[1].range, 5..15);
        assert_eq!(segs[1].fg, p().muted);
    }

    #[test]
    fn first_hit_wins_on_overlap() {
        // When two Hit spans overlap, the one with lower span index wins
        let spans = vec![
            Span { range: 0..10, style: SpanStyle::Hit(2) },
            Span { range: 5..15, style: SpanStyle::Hit(0) },
        ];
        let segs = segments(15, &spans, p().log_text, &p());

        // [0, 10): Hit(2) wins (lower span index = earlier span in input)
        assert_eq!(segs[0].range, 0..10);
        assert_eq!(segs[0].hit_bg, Some(p().hits[2].bg));
        assert_eq!(segs[0].fg, p().hits[2].fg);

        // [10, 15): Hit(0)
        assert_eq!(segs[1].range, 10..15);
        assert_eq!(segs[1].hit_bg, Some(p().hits[0].bg));
        assert_eq!(segs[1].fg, p().hits[0].fg);
    }
}

#[cfg(test)]
mod token_tests {
    use super::token_at;

    #[test]
    fn token_at_takes_the_word_under_the_index() {
        let line = "14:32:08  ERROR  timeout waiting for metrics";
        assert_eq!(token_at(line, 12).as_deref(), Some("ERROR"));
        assert_eq!(token_at(line, 10).as_deref(), Some("ERROR"), "start of a word");
        assert_eq!(token_at(line, 9), None, "whitespace");
        assert_eq!(token_at(line, line.len()).as_deref(), Some("metrics"), "end of the line belongs to the last word");
        assert_eq!(token_at("a=é1 b", 3).as_deref(), Some("a=é1"), "indices inside a multi-byte char snap back");
        assert_eq!(token_at("cut…", 4), None, "the truncation marker is not a word");
        assert_eq!(token_at("", 0), None);
    }
}
