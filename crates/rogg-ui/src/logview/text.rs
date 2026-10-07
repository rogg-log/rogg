use super::model::{Span, SpanStyle};
use crate::theme::{Color, Palette};
use std::ops::Range;

/// Lines longer than this are cut for display (spec §4.4); matching still sees the full line.
pub const MAX_DISPLAY_BYTES: usize = 64 * 1024;

/// The log view cuts decoded text only beyond this. Row producers cut raw lines at `MAX_DISPLAY_BYTES` before decoding
/// and mark the cut themselves; decoding can triple a line's size (each invalid byte becomes a 3-byte U+FFFD), so a
/// short line full of invalid bytes still fits.
pub const MAX_DISPLAY_TEXT_BYTES: usize = 3 * MAX_DISPLAY_BYTES + 3;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Segment {
    pub range: Range<usize>,
    pub fg: Color,
    pub hit_bg: Option<Color>,
    pub outline: bool,
    pub bold: bool,
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
        SpanStyle::Hit(_) | SpanStyle::Mark { .. } | SpanStyle::Bold | SpanStyle::SearchHit => None,
    }
}

fn hit_style(spans: &[Span], hits: &std::collections::BTreeSet<usize>, p: &Palette) -> Option<(Color, Color)> {
    match spans[*hits.iter().next()?].style {
        SpanStyle::Hit(slot) => {
            let h = p.hits[slot % p.hits.len()];
            Some((h.fg, h.bg))
        }
        SpanStyle::Mark { fg, bg } => Some((fg, bg)),
        _ => None,
    }
}

fn push_segment(out: &mut Vec<Segment>, seg: Segment) {
    if let Some(last) = out.last_mut()
        && last.range.end == seg.range.start
        && last.fg == seg.fg
        && last.hit_bg == seg.hit_bg
        && last.outline == seg.outline
        && last.bold == seg.bold
    {
        last.range.end = seg.range.end;
        return;
    }
    out.push(seg);
}

/// Splits `0..len` into maximal runs of identical style. Later foreground spans override earlier ones, a filter hit or
/// mark overrides the foreground (the first one wins), search hits add an outline and bold spans make the run bold.
///
/// O(n log n) event sweep: each span's start and end are visited once, in position order.
pub fn segments(len: usize, spans: &[Span], base: Color, p: &Palette) -> Vec<Segment> {
    use std::collections::BTreeSet;
    if len == 0 {
        return Vec::new();
    }
    let mut events = Vec::with_capacity(spans.len() * 2);
    for (idx, span) in spans.iter().enumerate() {
        let (start, end) = (span.range.start.min(len), span.range.end.min(len));
        if start < end {
            events.push((start, true, idx));
            events.push((end, false, idx));
        }
    }
    // Starts before ends at the same position; every event at a position is applied before the next run is emitted.
    events.sort_by_key(|e| (e.0, !e.1));
    let mut fg: BTreeSet<usize> = BTreeSet::new();
    let mut hits: BTreeSet<usize> = BTreeSet::new();
    let (mut outlines, mut bolds) = (0usize, 0usize);
    let mut out = Vec::new();
    let emit = |range: Range<usize>, fg: &BTreeSet<usize>, hits: &BTreeSet<usize>, outlines: usize, bolds: usize, out: &mut Vec<Segment>| {
        let base_fg = fg.iter().next_back().and_then(|&i| resolve(spans[i].style, p)).unwrap_or(base);
        let (color, hit_bg) = match hit_style(spans, hits, p) {
            Some((hit_fg, hit_bg)) => (hit_fg, Some(hit_bg)),
            None => (base_fg, None),
        };
        push_segment(out, Segment { range, fg: color, hit_bg, outline: outlines > 0, bold: bolds > 0 });
    };
    let mut last = 0;
    let mut i = 0;
    while i < events.len() {
        let pos = events[i].0;
        if last < pos {
            emit(last..pos, &fg, &hits, outlines, bolds, &mut out);
        }
        while i < events.len() && events[i].0 == pos {
            let (_, start, idx) = events[i];
            match spans[idx].style {
                SpanStyle::Hit(_) | SpanStyle::Mark { .. } => {
                    if start {
                        hits.insert(idx);
                    } else {
                        hits.remove(&idx);
                    }
                }
                SpanStyle::SearchHit => outlines = if start { outlines + 1 } else { outlines.saturating_sub(1) },
                SpanStyle::Bold => bolds = if start { bolds + 1 } else { bolds.saturating_sub(1) },
                SpanStyle::Faint | SpanStyle::Muted | SpanStyle::Error | SpanStyle::Amber | SpanStyle::Fg(_) => {
                    if start {
                        fg.insert(idx);
                    } else {
                        fg.remove(&idx);
                    }
                }
            }
            i += 1;
        }
        last = pos;
    }
    if last < len {
        emit(last..len, &fg, &hits, outlines, bolds, &mut out);
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
        assert_eq!(segments(5, &[], p().log_text, &p()), vec![Segment { range: 0..5, fg: p().log_text, hit_bg: None, outline: false, bold: false }]);
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
                let seg = segs.iter().find(|s| s.range.contains(&pos)).unwrap_or_else(|| panic!("no segment at {pos}"));
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

    #[test]
    fn marks_act_like_hits_and_bold_is_tracked() {
        let fg = Color::rgb(0x112233);
        let bg = Color::rgb(0x445566);
        let spans = vec![
            Span { range: 0..4, style: SpanStyle::Mark { fg, bg } },
            Span { range: 2..6, style: SpanStyle::Hit(1) },
            Span { range: 3..8, style: SpanStyle::Bold },
        ];
        let segs = segments(8, &spans, p().log_text, &p());
        assert_eq!(
            segs,
            vec![
                Segment { range: 0..3, fg, hit_bg: Some(bg), outline: false, bold: false },
                Segment { range: 3..4, fg, hit_bg: Some(bg), outline: false, bold: true },
                Segment { range: 4..6, fg: p().hits[1].fg, hit_bg: Some(p().hits[1].bg), outline: false, bold: true },
                Segment { range: 6..8, fg: p().log_text, hit_bg: None, outline: false, bold: true },
            ]
        );
    }

    #[test]
    fn later_foreground_spans_override_a_line_colour() {
        let line = Color::rgb(0xff0000);
        let faint = p().faint;
        let spans = vec![Span { range: 0..10, style: SpanStyle::Fg(line) }, Span { range: 0..3, style: SpanStyle::Faint }];
        let segs = segments(10, &spans, p().log_text, &p());
        assert_eq!(segs[0].range, 0..3);
        assert_eq!(segs[0].fg, faint);
        assert_eq!((segs[1].range.clone(), segs[1].fg), (3..10, line));
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
