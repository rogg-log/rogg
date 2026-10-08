use gpui_kit::prelude::*;
use gpui_kit::{Svg, Transformation, radians, svg};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IconName {
    File,
    Folder,
    Search,
    Wrap,
    Pin,
    PinFilled,
    Filter,
    Follow,
    Check,
    Off,
    Minus,
    History,
    Edit,
    Export,
    Sun,
    Settings,
    Side,
    OpenTab,
    Pause,
    Close,
    ChevronDown,
    ChevronRight,
    /// Heavy chevron matching the native `<select>` arrow of the reference.
    SelectCaret,
}

impl IconName {
    pub const ALL: [IconName; 23] = [
        IconName::File, IconName::Folder, IconName::Search, IconName::Wrap, IconName::Pin, IconName::PinFilled,
        IconName::Filter, IconName::Follow, IconName::Check, IconName::Off, IconName::Minus, IconName::History,
        IconName::Edit, IconName::Export, IconName::Sun, IconName::Settings, IconName::Side, IconName::OpenTab,
        IconName::Pause, IconName::Close, IconName::ChevronDown, IconName::ChevronRight, IconName::SelectCaret,
    ];

    pub fn path(self) -> &'static str {
        match self {
            IconName::File => "icons/rogg/file.svg",
            IconName::Folder => "icons/rogg/folder.svg",
            IconName::Search => "icons/rogg/search.svg",
            IconName::Wrap => "icons/rogg/wrap.svg",
            IconName::Pin => "icons/rogg/pin.svg",
            IconName::PinFilled => "icons/rogg/pin-filled.svg",
            IconName::Filter => "icons/rogg/filter.svg",
            IconName::Follow => "icons/rogg/follow.svg",
            IconName::Check => "icons/rogg/check.svg",
            IconName::Off => "icons/rogg/off.svg",
            IconName::Minus => "icons/rogg/minus.svg",
            IconName::History => "icons/rogg/history.svg",
            IconName::Edit => "icons/rogg/edit.svg",
            IconName::Export => "icons/rogg/export.svg",
            IconName::Sun => "icons/rogg/sun.svg",
            IconName::Settings => "icons/rogg/settings.svg",
            IconName::Side => "icons/rogg/side.svg",
            IconName::OpenTab => "icons/rogg/open-tab.svg",
            IconName::Pause => "icons/rogg/pause.svg",
            IconName::Close => "icons/rogg/close.svg",
            IconName::ChevronDown => "icons/rogg/chevron-down.svg",
            IconName::ChevronRight => "icons/rogg/chevron-right.svg",
            IconName::SelectCaret => "icons/rogg/select-caret.svg",
        }
    }
}

/// An icon element. Always set `.size(..)` and `.text_color(..)`: gpui svgs paint only with an explicit text color.
pub fn icon(name: IconName) -> Svg {
    svg().path(name.path()).flex_shrink_0()
}

/// The disclosure chevron: points right when closed and is rotated 90° clockwise (pointing down) when open.
pub fn chevron(open: bool) -> Svg {
    let s = icon(IconName::ChevronRight);
    if open { s.with_transformation(Transformation::rotate(radians(std::f32::consts::FRAC_PI_2))) } else { s }
}
