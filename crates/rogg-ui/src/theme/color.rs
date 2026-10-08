use gpui_kit::{Background, Fill, Hsla, rgba};

/// An sRGB color stored as `0xRRGGBBAA`, so tests can compare exact values.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Color(pub u32);

impl Color {
    pub const TRANSPARENT: Color = Color(0);

    pub const fn rgb(hex: u32) -> Self {
        Color((hex << 8) | 0xff)
    }

    pub const fn rgba(hex: u32) -> Self {
        Color(hex)
    }

    pub const fn with_alpha(self, alpha: u8) -> Self {
        Color((self.0 & 0xffff_ff00) | alpha as u32)
    }

    pub fn hsla(self) -> Hsla {
        rgba(self.0).into()
    }

    /// The colour as `0xRRGGBBAA`, rounding each channel to 8 bits.
    pub fn from_hsla(h: Hsla) -> Color {
        let c = gpui_kit::Rgba::from(h);
        let ch = |v: f32| (v.clamp(0., 1.) * 255.).round() as u32;
        Color((ch(c.r) << 24) | (ch(c.g) << 16) | (ch(c.b) << 8) | ch(c.a))
    }

    /// Parses `#rgb`, `#rgba`, `#rrggbb` and `#rrggbbaa`.
    pub fn parse_css(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let doubled = |h: &str| h.chars().flat_map(|c| [c, c]).collect::<String>();
        let full = match hex.len() {
            3 => format!("{}ff", doubled(hex)),
            4 => doubled(hex),
            6 => format!("{hex}ff"),
            8 => hex.to_string(),
            _ => return None,
        };
        u32::from_str_radix(&full, 16).ok().map(Color)
    }
}

impl From<Color> for Hsla {
    fn from(c: Color) -> Self {
        c.hsla()
    }
}

impl From<Color> for Background {
    fn from(c: Color) -> Self {
        c.hsla().into()
    }
}

impl From<Color> for Fill {
    fn from(c: Color) -> Self {
        c.hsla().into()
    }
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn parses_all_css_hex_forms() {
        assert_eq!(Color::parse_css("#191c1f"), Some(Color(0x191c1fff)));
        assert_eq!(Color::parse_css("#8f352e10"), Some(Color(0x8f352e10)));
        assert_eq!(Color::parse_css("#0007"), Some(Color(0x00000077)));
        assert_eq!(Color::parse_css("#fff"), Some(Color(0xffffffff)));
        assert_eq!(Color::parse_css("191c1f"), None);
        assert_eq!(Color::parse_css("#12"), None);
        assert_eq!(Color::parse_css("#zzzzzz"), None);
    }

    #[test]
    fn from_hsla_round_trips_8_bit_colours() {
        for c in [0xff0000ff, 0xffffffff, 0x000000ff, 0x336699ff, 0x3fa7ff80, 0x00000000] {
            assert_eq!(Color::from_hsla(Color(c).hsla()), Color(c), "{c:#010x}");
        }
    }

    #[test]
    fn rgb_sets_opaque_alpha_and_with_alpha_replaces_it() {
        assert_eq!(Color::rgb(0xa8c5b7), Color(0xa8c5b7ff));
        assert_eq!(Color::rgb(0xa8c5b7).with_alpha(0x80), Color(0xa8c5b780));
    }
}
