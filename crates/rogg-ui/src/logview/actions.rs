//! Keyboard actions handled by a focused log view. The app binds keys to them in the `LogView` context.

gpui_kit::actions!(
    log_view,
    [
        SelectPrev,
        SelectNext,
        ExtendPrev,
        ExtendNext,
        PagePrev,
        PageNext,
        SelectFirst,
        SelectLast,
        CopySelection,
        CopySelectionWithNumbers
    ]
);

/// Key context of a focused log view.
pub const KEY_CONTEXT: &str = "LogView";
