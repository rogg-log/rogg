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
pub fn segments(len: usize, spans: &[Span], base: Color, p: &Palette) -> Vec<Segment> {
    let mut cuts = vec![0, len];
    for s in spans {
        cuts.push(s.range.start.min(len));
        cuts.push(s.range.end.min(len));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<Segment> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a >= b {
            continue;
        }
        let mut fg = base;
        let mut hit: Option<(Color, Color)> = None;
        let mut outline = false;
        for s in spans.iter().filter(|s| s.range.start <= a && s.range.end >= b) {
            match s.style {
                SpanStyle::Hit(slot) => {
                    let h = p.hits[slot % p.hits.len()];
                    hit.get_or_insert((h.fg, h.bg));
                }
                SpanStyle::SearchHit => outline = true,
                other => fg = resolve(other, p).unwrap_or(fg),
            }
        }
        let (fg, hit_bg) = match hit {
            Some((hfg, hbg)) => (hfg, Some(hbg)),
            None => (fg, None),
        };
        if let Some(last) = out.last_mut() {
            if last.range.end == a && last.fg == fg && last.hit_bg == hit_bg && last.outline == outline {
                last.range.end = b;
                continue;
            }
        }
        out.push(Segment { range: a..b, fg, hit_bg, outline });
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
}
