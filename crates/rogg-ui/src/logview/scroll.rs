/// Scroll position as "row `row`, scrolled `offset` px past its top". Row 0 may have a negative offset
/// down to `-pad`, so the first row starts below the list's top padding. Independent of total content
/// height, so it works for millions of rows and for unknown wrapped heights.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScrollAnchor {
    pub row: usize,
    pub offset: f32,
}

pub struct ScrollGeometry<'a> {
    pub row_count: usize,
    pub viewport: f32,
    pub pad: f32,
    pub height_of: &'a dyn Fn(usize) -> f32,
}

fn before(a: ScrollAnchor, b: ScrollAnchor) -> bool {
    a.row < b.row || (a.row == b.row && a.offset < b.offset)
}

impl ScrollGeometry<'_> {
    pub fn top(&self) -> ScrollAnchor {
        ScrollAnchor { row: 0, offset: -self.pad }
    }

    /// Whether `a` shows the end of the content (the last row and the bottom padding); true when everything fits.
    pub fn is_at_bottom(&self, a: ScrollAnchor) -> bool {
        self.row_count == 0 || !before(a, self.bottom())
    }

    /// The furthest anchor: last row's bottom plus padding meets the viewport bottom.
    pub fn bottom(&self) -> ScrollAnchor {
        let mut remaining = self.viewport - self.pad;
        for row in (0..self.row_count).rev() {
            let h = (self.height_of)(row);
            if remaining <= h {
                return ScrollAnchor { row, offset: h - remaining };
            }
            remaining -= h;
        }
        if remaining <= self.pad && self.row_count > 0 {
            ScrollAnchor { row: 0, offset: -remaining }
        } else {
            self.top()
        }
    }

    pub fn clamp(&self, a: ScrollAnchor) -> ScrollAnchor {
        let (top, bottom) = (self.top(), self.bottom());
        if self.row_count == 0 || before(a, top) {
            top
        } else if before(bottom, a) {
            bottom
        } else {
            a
        }
    }

    /// Moves the anchor to the row that actually contains its offset (walks only rows crossed).
    fn normalize(&self, mut a: ScrollAnchor) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        a.row = a.row.min(self.row_count - 1);
        loop {
            if a.offset < 0. && a.row > 0 {
                a.row -= 1;
                a.offset += (self.height_of)(a.row);
                continue;
            }
            let h = (self.height_of)(a.row);
            if a.offset >= h && a.row + 1 < self.row_count {
                a.offset -= h;
                a.row += 1;
                continue;
            }
            return a;
        }
    }

    pub fn scroll_by(&self, a: ScrollAnchor, delta: f32) -> ScrollAnchor {
        self.clamp(self.normalize(ScrollAnchor { row: a.row, offset: a.offset + delta }))
    }

    pub fn center_on(&self, row: usize) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        let row = row.min(self.row_count - 1);
        let h = (self.height_of)(row);
        self.clamp(self.normalize(ScrollAnchor { row, offset: -(self.viewport - h) / 2. }))
    }

    /// Anchor with `row` at the top of the viewport (clamped at the end of the content). Row 0 keeps the list's
    /// top padding, like `top()`.
    pub fn row_top(&self, row: usize) -> ScrollAnchor {
        if self.row_count == 0 || row == 0 {
            return self.top();
        }
        self.clamp(ScrollAnchor { row: row.min(self.row_count - 1), offset: 0. })
    }

    /// The smallest scroll that shows all of `row`: unchanged when it is visible, otherwise aligned with the
    /// nearer viewport edge. Walks only the rows between the anchor and the target that fit in the viewport.
    pub fn ensure_visible(&self, a: ScrollAnchor, row: usize) -> ScrollAnchor {
        if self.row_count == 0 {
            return self.top();
        }
        let row = row.min(self.row_count - 1);
        if row < a.row || (row == a.row && a.offset > 0.) {
            return self.row_top(row);
        }
        let h = (self.height_of)(row);
        let mut y = -a.offset;
        let mut r = a.row;
        while r < row && y <= self.viewport {
            y += (self.height_of)(r);
            r += 1;
        }
        if r == row && y + h <= self.viewport {
            return a;
        }
        if row == self.row_count - 1 {
            // The last row also shows the list's bottom padding.
            return self.bottom();
        }
        self.clamp(self.normalize(ScrollAnchor { row, offset: h - self.viewport }))
    }

    /// Rows intersecting the viewport with their top y relative to the viewport top.
    pub fn visible(&self, a: ScrollAnchor) -> Vec<(usize, f32)> {
        let mut out = Vec::new();
        let mut y = -a.offset;
        let mut row = a.row;
        while row < self.row_count && y < self.viewport {
            out.push((row, y));
            y += (self.height_of)(row);
            row += 1;
        }
        out
    }

    /// Inverse of `thumb`: the anchor whose thumb top is at fraction `pos` of the content (0 = top).
    pub fn at_position(&self, pos: f32) -> ScrollAnchor {
        if self.row_count == 0 || pos <= 0. {
            return self.top();
        }
        let exact = pos.min(1.) * self.row_count as f32;
        let row = (exact.floor() as usize).min(self.row_count - 1);
        let offset = (exact - row as f32) * (self.height_of)(row);
        self.clamp(ScrollAnchor { row, offset })
    }

    /// Scrollbar thumb as (top fraction, length fraction); None when everything fits.
    pub fn thumb(&self, a: ScrollAnchor) -> Option<(f32, f32)> {
        if self.row_count == 0 || self.bottom() == self.top() {
            return None;
        }
        let h = (self.height_of)(a.row).max(1.);
        let count = self.row_count as f32;
        let len = (self.viewport / h / count).min(1.);
        let pos = ((a.row as f32 + (a.offset / h).max(0.)) / count).clamp(0., 1. - len);
        Some((pos, len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo<'a>(rows: usize, viewport: f32, h: &'a dyn Fn(usize) -> f32) -> ScrollGeometry<'a> {
        ScrollGeometry { row_count: rows, viewport, pad: 4., height_of: h }
    }

    #[test]
    fn top_starts_rows_below_padding() {
        let h = |_| 23.;
        let g = geo(100, 230., &h);
        let v = g.visible(g.top());
        assert_eq!(v[0], (0, 4.));
        assert_eq!(v[1], (1, 27.));
        assert_eq!(v.len(), 10); // rows at 4..211; row 10 would start at 234 > 230
    }

    #[test]
    fn scroll_by_moves_across_rows_and_clamps() {
        let h = |_| 23.;
        let g = geo(100, 230., &h);
        let a = g.scroll_by(g.top(), 50.);
        assert_eq!(a, ScrollAnchor { row: 2, offset: 0. }); // -4 + 50 = 46 = 2 rows
        assert_eq!(g.scroll_by(a, -1000.), g.top());
        let end = g.scroll_by(a, 1e6);
        assert_eq!(end, g.bottom());
        // last row bottom + pad == viewport bottom
        let v = g.visible(end);
        let (last, y) = *v.last().unwrap();
        assert_eq!(last, 99);
        assert!((y + 23. + 4. - 230.).abs() < 1e-3);
    }

    #[test]
    fn short_content_never_scrolls() {
        let h = |_| 23.;
        let g = geo(3, 230., &h);
        assert_eq!(g.bottom(), g.top());
        assert_eq!(g.scroll_by(g.top(), 500.), g.top());
        assert!(g.thumb(g.top()).is_none());
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        let h = |_| 23.;
        let empty = geo(0, 230., &h);
        assert!(empty.visible(empty.top()).is_empty());
        assert_eq!(empty.scroll_by(empty.top(), 100.), empty.top());
        assert_eq!(empty.center_on(0), empty.top());
        let tiny = geo(10, 5., &h);
        let a = tiny.scroll_by(tiny.top(), 40.);
        assert!(!tiny.visible(a).is_empty());
        let one = geo(1, 230., &h);
        assert_eq!(one.center_on(0), one.top());
    }

    #[test]
    fn center_on_places_row_mid_viewport_and_clamps_at_edges() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let a = g.center_on(500);
        let (_, y) = *g.visible(a).iter().find(|(r, _)| *r == 500).unwrap();
        assert!((y + 11.5 - 115.).abs() < 1e-3, "row 500 centred, got y={y}");
        assert_eq!(g.center_on(0), g.top());
        assert_eq!(g.center_on(999), g.bottom());
    }

    #[test]
    fn mixed_heights_gap_rows() {
        let h = |r: usize| if r % 2 == 1 { 17. } else { 23. };
        let g = geo(10, 100., &h);
        let v = g.visible(g.top());
        assert_eq!(v[..3], [(0, 4.), (1, 27.), (2, 44.)]);
        let a = g.scroll_by(g.top(), 4. + 23. + 17.);
        assert_eq!(a, ScrollAnchor { row: 2, offset: 0. });
    }

    #[test]
    fn thumb_tracks_position() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let (top, len) = g.thumb(g.top()).unwrap();
        assert_eq!(top, 0.);
        assert!((len - 0.01).abs() < 1e-3);
        let (top, _) = g.thumb(g.center_on(500)).unwrap();
        assert!(top > 0.45 && top < 0.55);
    }

    #[test]
    fn ensure_visible_is_minimal() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let top = g.top();
        assert_eq!(g.ensure_visible(top, 5), top, "already visible rows don't scroll");
        let down = g.ensure_visible(top, 300);
        let (_, y) = *g.visible(down).iter().find(|(r, _)| *r == 300).unwrap();
        assert!((y + 23. - 230.).abs() < 1e-3, "row 300 bottom-aligned, got y={y}");
        let up = g.ensure_visible(down, 100);
        assert_eq!(up, ScrollAnchor { row: 100, offset: 0. }, "rows above align to the top");
        assert_eq!(g.ensure_visible(top, 5000), g.bottom(), "out-of-range rows clamp");
    }

    #[test]
    fn row_top_puts_the_row_first() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        assert_eq!(g.row_top(40), ScrollAnchor { row: 40, offset: 0. });
        assert_eq!(g.row_top(999), g.bottom());
        assert_eq!(g.row_top(0), g.top(), "the first row keeps the top padding");
    }

    #[test]
    fn ensure_visible_row_zero_returns_to_the_padded_top() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        let scrolled = ScrollAnchor { row: 300, offset: 7. };
        assert_eq!(g.ensure_visible(scrolled, 0), g.top());
        let slightly = ScrollAnchor { row: 0, offset: 5. };
        assert_eq!(g.ensure_visible(slightly, 0), g.top());
    }

    #[test]
    fn at_position_inverts_thumb() {
        let h = |_| 23.;
        let g = geo(1000, 230., &h);
        assert_eq!(g.at_position(0.), g.top());
        let a = g.at_position(0.5);
        assert_eq!(a.row, 500);
        let (pos, _) = g.thumb(a).unwrap();
        assert!((pos - 0.5).abs() < 1e-3);
        assert_eq!(g.at_position(1.), g.bottom());
    }
}
