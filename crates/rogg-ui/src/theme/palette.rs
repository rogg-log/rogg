use super::color::Color;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HitColor {
    pub fg: Color,
    pub bg: Color,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChipColor {
    pub border: Color,
    pub bg: Color,
    pub term: Color,
}

/// Every color the UI paints, transcribed from the reference CSS (`:root`, `.light`, and component rules).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Palette {
    pub page: Color,
    pub bg: Color,
    pub side: Color,
    pub raised: Color,
    pub hover: Color,
    pub border: Color,
    pub line: Color,
    pub text: Color,
    pub log_text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub accent_bg: Color,
    pub accent_border: Color,
    pub error: Color,
    pub error_bg: Color,
    pub amber: Color,
    pub amber_bg: Color,
    pub selection: Color,
    pub error_row_tint: Color,
    pub band_border: Color,
    pub band_handle: Color,
    pub scrollbar_thumb: Color,
    pub chip_divider: Color,
    pub swatch_border: Color,
    pub backdrop: Color,
    pub dialog_shadow: Color,
    pub traffic: [Color; 3],
    pub hits: [HitColor; 4],
    pub chips: [ChipColor; 4],
}

const fn hit(fg: u32, bg: u32) -> HitColor {
    HitColor { fg: Color::rgb(fg), bg: Color::rgb(bg) }
}

const fn chip(border: u32, bg: u32, term: u32) -> ChipColor {
    ChipColor { border: Color::rgb(border), bg: Color::rgb(bg), term: Color::rgb(term) }
}

impl Palette {
    pub const fn dark() -> Self {
        Palette {
            page: Color::rgb(0x111315),
            bg: Color::rgb(0x191c1f),
            side: Color::rgb(0x1e2125),
            raised: Color::rgb(0x25292e),
            hover: Color::rgb(0x2c3137),
            border: Color::rgb(0x34393f),
            line: Color::rgb(0x292e33),
            text: Color::rgb(0xe2e6e9),
            log_text: Color::rgb(0xbbc5ce),
            muted: Color::rgb(0x929da7),
            faint: Color::rgb(0x67737e),
            accent: Color::rgb(0xa8c5b7),
            accent_bg: Color::rgb(0x263e34),
            accent_border: Color::rgb(0x50665c),
            error: Color::rgb(0xefaaa2),
            error_bg: Color::rgb(0x49302f),
            amber: Color::rgb(0xe0c283),
            amber_bg: Color::rgb(0x443b28),
            selection: Color::rgb(0x263844),
            error_row_tint: Color::rgba(0x8f352e10),
            band_border: Color::rgb(0x49534f),
            band_handle: Color::rgb(0x65776e),
            scrollbar_thumb: Color::rgb(0x4b545d),
            chip_divider: Color::rgba(0xffffff15),
            swatch_border: Color::rgba(0xffffff20),
            backdrop: Color::rgba(0x00000077),
            dialog_shadow: Color::rgba(0x00000077),
            traffic: [Color::rgb(0xec6a60), Color::rgb(0xe5bb55), Color::rgb(0x69bd63)],
            hits: [
                hit(0xffc4bb, 0x744037),
                hit(0xf2d693, 0x64502b),
                hit(0xacd7ea, 0x315064),
                hit(0xcbb5eb, 0x514169),
            ],
            chips: [
                chip(0x67514b, 0x362b29, 0xefaaa2),
                chip(0x64583d, 0x342f24, 0xe0c283),
                chip(0x415762, 0x263037, 0xa2c9d8),
                chip(0x5c4c6c, 0x302a37, 0xc2acd9),
            ],
        }
    }

    /// Light values come from `.light{...}` and `.light ...` rules; anything the reference leaves unset reuses dark.
    pub const fn light() -> Self {
        let dark = Palette::dark();
        Palette {
            page: Color::rgb(0xe9ecea),
            bg: Color::rgb(0xfcfcfa),
            side: Color::rgb(0xf2f4f1),
            raised: Color::rgb(0xe7ebe7),
            hover: Color::rgb(0xe7ece7),
            border: Color::rgb(0xd1d8d1),
            line: Color::rgb(0xe5e9e3),
            text: Color::rgb(0x29372d),
            log_text: Color::rgb(0x435047),
            muted: Color::rgb(0x69766c),
            faint: Color::rgb(0x849086),
            accent: Color::rgb(0x386a4f),
            accent_bg: Color::rgb(0xe0ece3),
            error: Color::rgb(0xa44437),
            amber: Color::rgb(0x9a6c20),
            selection: Color::rgb(0xe1ece6),
            hits: [
                hit(0x763227, 0xf1c7bc),
                hit(0x714c16, 0xead49b),
                hit(0x325e70, 0xbfdae6),
                hit(0x624580, 0xd9c9eb),
            ],
            chips: [
                chip(0xdcc4bc, 0xf8eae6, 0xa44437),
                chip(0xd8cba6, 0xf7f0dd, 0x9a6c20),
                chip(0xb9d0da, 0xe7f0f4, 0x426b7b),
                chip(0xcdbdda, 0xece5f5, 0x776083),
            ],
            ..dark
        }
    }

    /// All tokens with display names, for the gallery's token page.
    pub fn named(&self) -> Vec<(&'static str, Color)> {
        let mut v = vec![
            ("page", self.page), ("bg", self.bg), ("side", self.side), ("raised", self.raised),
            ("hover", self.hover), ("border", self.border), ("line", self.line), ("text", self.text),
            ("log text", self.log_text), ("muted", self.muted), ("faint", self.faint), ("accent", self.accent),
            ("accent-bg", self.accent_bg), ("accent-border", self.accent_border), ("error", self.error),
            ("error-bg", self.error_bg), ("amber", self.amber), ("amber-bg", self.amber_bg),
            ("selection", self.selection), ("error row tint", self.error_row_tint),
            ("band border", self.band_border), ("band handle", self.band_handle),
            ("scrollbar thumb", self.scrollbar_thumb), ("chip divider", self.chip_divider),
            ("swatch border", self.swatch_border), ("backdrop", self.backdrop),
            ("dialog shadow", self.dialog_shadow), ("traffic red", self.traffic[0]),
            ("traffic yellow", self.traffic[1]), ("traffic green", self.traffic[2]),
        ];
        const HIT: [(&str, &str); 4] = [("hit-0 fg", "hit-0 bg"), ("hit-1 fg", "hit-1 bg"), ("hit-2 fg", "hit-2 bg"), ("hit-3 fg", "hit-3 bg")];
        const CHIP: [(&str, &str, &str); 4] = [
            ("chip-0 border", "chip-0 bg", "chip-0 term"),
            ("chip-1 border", "chip-1 bg", "chip-1 term"),
            ("chip-2 border", "chip-2 bg", "chip-2 term"),
            ("chip-3 border", "chip-3 bg", "chip-3 term"),
        ];
        for (i, (fg, bg)) in HIT.iter().enumerate() {
            v.push((fg, self.hits[i].fg));
            v.push((bg, self.hits[i].bg));
        }
        for (i, (b, bg, t)) in CHIP.iter().enumerate() {
            v.push((b, self.chips[i].border));
            v.push((bg, self.chips[i].bg));
            v.push((t, self.chips[i].term));
        }
        v
    }
}
