//! The highlighting profile editor (spec §6.1): choose, create, duplicate, rename, delete, import and export profiles;
//! reorder, enable, add, edit and delete rules; edit the filter-match palette. Every change is saved at once and open
//! tabs restyle (the workspace listens for `Changed`).

use crate::profiles::{ProfileStore, RowStyler};
use crate::tab::Tab;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, BorrowAppContext, Context, Entity, EventEmitter, SharedString, Subscription, TestSupportExt, WeakEntity, Window, div, px};
use rogg_core::profile::{DEFAULT_PROFILE, LineRule, Profile, default_palette, names_collide};
use rogg_ui::controls::{ActionButton, CompactSelect};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{EditableRuleRow, PaletteSlot, SectionLabel, TextField};
use rogg_ui::theme::ActiveRoggTheme;

pub enum ProfileEditorEvent {
    Close,
    /// Profiles changed: restyle tabs (and move tabs whose profile is gone to Default).
    Changed,
    /// A profile was renamed: tabs and the default-profile setting follow.
    Renamed { from: String, to: String },
    Toast(SharedString),
}

pub struct ProfileEditor {
    tab: Option<WeakEntity<Tab>>,
    profile: String,
    selected_rule: Option<usize>,
    selected_slot: Option<usize>,
    rename: Option<(Entity<InputState>, Subscription)>,
    rename_error: Option<SharedString>,
}

impl EventEmitter<ProfileEditorEvent> for ProfileEditor {}

/// A callback that runs `f` on the editor.
fn ed(this: &Entity<ProfileEditor>, f: impl Fn(&mut ProfileEditor, &mut Window, &mut Context<ProfileEditor>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |e, cx| f(e, window, cx))
}

impl ProfileEditor {
    pub fn new(tab: Option<WeakEntity<Tab>>, profile: String) -> Self {
        Self { tab, profile, selected_rule: None, selected_slot: None, rename: None, rename_error: None }
    }

    pub fn profile_name(&self) -> &str {
        &self.profile
    }

    pub fn selected_rule(&self) -> Option<usize> {
        self.selected_rule
    }

    pub fn selected_slot(&self) -> Option<usize> {
        self.selected_slot
    }

    pub fn rename_input(&self) -> Option<Entity<InputState>> {
        self.rename.as_ref().map(|(input, _)| input.clone())
    }

    /// Edits the shown profile, saves it and tells the workspace.
    pub(crate) fn edit(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Profile)) {
        let name = self.profile.clone();
        cx.update_global::<ProfileStore, _>(|s, _| s.edit(&name, f));
        cx.emit(ProfileEditorEvent::Changed);
        cx.notify();
    }

    fn rule_count(&self, cx: &App) -> usize {
        cx.global::<ProfileStore>().get(&self.profile).rules.len()
    }

    /// Shows `name` and assigns it to the tab the editor was opened from (spec §6.1: each tab has one profile).
    pub fn select_profile(&mut self, name: &str, cx: &mut Context<Self>) {
        self.profile = name.to_string();
        self.selected_rule = None;
        self.selected_slot = None;
        self.rename = None;
        self.rename_error = None;
        if let Some(tab) = self.tab.as_ref().and_then(WeakEntity::upgrade) {
            let name = SharedString::from(name.to_string());
            tab.update(cx, |t, cx| t.set_profile(name, cx));
        }
        cx.notify();
    }

    pub fn new_profile(&mut self, cx: &mut Context<Self>) {
        let name = cx.update_global::<ProfileStore, _>(|s, _| s.add(Profile { name: "New profile".into(), rules: Vec::new(), hit_palette: default_palette() }));
        cx.emit(ProfileEditorEvent::Changed);
        self.select_profile(&name, cx);
    }

    pub fn duplicate(&mut self, cx: &mut Context<Self>) {
        let mut copy = cx.global::<ProfileStore>().get(&self.profile).clone();
        copy.name = format!("{} copy", copy.name);
        let name = cx.update_global::<ProfileStore, _>(|s, _| s.add(copy));
        cx.emit(ProfileEditorEvent::Changed);
        self.select_profile(&name, cx);
    }

    /// Deletes the shown profile (never Default); the tab moves to Default.
    pub fn delete(&mut self, cx: &mut Context<Self>) -> bool {
        let name = self.profile.clone();
        if !cx.update_global::<ProfileStore, _>(|s, _| s.remove(&name)) {
            return false;
        }
        cx.emit(ProfileEditorEvent::Changed);
        self.select_profile(DEFAULT_PROFILE, cx);
        true
    }

    /// Opens the name field; Default cannot be renamed (it must always exist, like it cannot be deleted).
    pub fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.profile == DEFAULT_PROFILE {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(self.profile.clone()));
        input.update(cx, |i, cx| i.focus(window, cx));
        let sub = cx.subscribe_in(&input, window, |e, _, ev: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                e.apply_rename(window, cx);
            }
        });
        self.rename = Some((input, sub));
        self.rename_error = None;
        cx.notify();
    }

    /// Renames the shown profile to the input's text; an empty or taken name shows an error instead. Default is never
    /// renamed.
    pub fn apply_rename(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some((input, _)) = &self.rename else { return };
        if self.profile == DEFAULT_PROFILE {
            self.rename = None;
            cx.notify();
            return;
        }
        let to = input.read(cx).value().trim().to_string();
        let from = self.profile.clone();
        if to == from {
            self.rename = None;
            cx.notify();
            return;
        }
        // Names that would share a file on the case-insensitive disk are taken too; the profile's own name is not
        // (so "Ops" → "ops" is allowed).
        let taken = cx.global::<ProfileStore>().names().iter().any(|n| *n != from && names_collide(n, &to));
        if to.is_empty() || taken {
            self.rename_error = Some(if to.is_empty() { "Type a name".into() } else { format!("A profile named {to} already exists").into() });
            cx.notify();
            return;
        }
        let new_name = to.clone();
        cx.update_global::<ProfileStore, _>(|s, _| s.edit(&from, |p| p.name = new_name));
        self.profile = to.clone();
        self.rename = None;
        self.rename_error = None;
        cx.emit(ProfileEditorEvent::Renamed { from, to });
        cx.emit(ProfileEditorEvent::Changed);
        cx.notify();
    }

    pub fn toggle_rule(&mut self, i: usize, cx: &mut Context<Self>) {
        self.edit(cx, |p| {
            if let Some(r) = p.rules.get_mut(i) {
                r.enabled = !r.enabled;
            }
        });
    }

    pub fn move_rule(&mut self, i: usize, up: bool, cx: &mut Context<Self>) {
        let n = self.rule_count(cx);
        let j = if up { i.checked_sub(1) } else { (i + 1 < n).then_some(i + 1) };
        let Some(j) = j else { return };
        self.edit(cx, |p| p.rules.swap(i, j));
        if self.selected_rule == Some(i) {
            self.selected_rule = Some(j);
        } else if self.selected_rule == Some(j) {
            self.selected_rule = Some(i);
        }
    }

    pub fn add_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |p| p.rules.push(LineRule::new("")));
        let last = self.rule_count(cx) - 1;
        self.select_rule(Some(last), window, cx);
    }

    pub fn delete_rule(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |p| {
            if i < p.rules.len() {
                p.rules.remove(i);
            }
        });
        self.select_rule(None, window, cx);
    }

    /// Selects rule `i` (its editing form opens below it in Task 14).
    pub fn select_rule(&mut self, i: Option<usize>, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_rule = i;
        cx.notify();
    }

    pub fn select_slot(&mut self, i: Option<usize>, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_slot = i;
        cx.notify();
    }
}

impl Render for ProfileEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let store = cx.global::<ProfileStore>();
        let profile = store.get(&self.profile).clone();
        let errors = store.errors(&self.profile).to_vec();
        let names = store.names();
        let theme = cx.rogg().clone();
        let styler = RowStyler::new(&profile, store.compiled(&self.profile), &theme.palette, theme.appearance);
        let selected = names.iter().position(|n| *n == profile.name).unwrap_or(0);
        let pick_names = names.clone();
        let mut toolbar = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(6.))
            .mt(px(10.))
            .child(CompactSelect::new("profile-select", names.iter().map(|n| SharedString::from(n.clone())).collect(), selected).on_select({
                let this = this.clone();
                move |i, _, cx| {
                    if let Some(name) = pick_names.get(i) {
                        let name = name.clone();
                        this.update(cx, |e, cx| e.select_profile(&name, cx));
                    }
                }
            }))
            .child(ActionButton::new("profile-new", "New").on_click(ed(&this, |e, _, cx| e.new_profile(cx))))
            .child(ActionButton::new("profile-duplicate", "Duplicate").on_click(ed(&this, |e, _, cx| e.duplicate(cx))))
            .when(profile.name != DEFAULT_PROFILE, |d| {
                d.child(ActionButton::new("profile-rename", "Rename").on_click(ed(&this, |e, window, cx| e.start_rename(window, cx))))
            })
            .child(ActionButton::new("profile-delete", "Delete").on_click(ed(&this, |e, _, cx| {
                e.delete(cx);
            })))
            .child(ActionButton::new("profile-import", "Import…").on_click(ed(&this, |e, window, cx| e.import(window, cx))))
            .child(ActionButton::new("profile-export", "Export…").on_click(ed(&this, |e, window, cx| e.export(window, cx))));
        if let Some((input, _)) = &self.rename {
            toolbar = toolbar
                .child(TextField::new("profile-name", input.clone()).width(160.).error(self.rename_error.is_some()))
                .child(ActionButton::new("profile-rename-apply", "Rename").primary().on_click(ed(&this, |e, window, cx| e.apply_rename(window, cx))));
        }
        let mut frame = DialogFrame::new(format!("Highlighting · {}", profile.name))
            .description("Rules apply top to bottom; for each effect the first matching rule wins. Changes apply immediately.")
            .on_close(ed(&this, |_, _, cx| cx.emit(ProfileEditorEvent::Close)))
            .child(toolbar);
        if let Some(error) = &self.rename_error {
            frame = frame.child(div().id("profile-rename-error").test_support().mt(px(4.)).text_size(theme.text_scale().px(10.)).text_color(theme.palette.error).child(error.clone()));
        }
        frame = frame.child(SectionLabel::new("Rules"));
        let count = profile.rules.len();
        for (i, rule) in profile.rules.iter().enumerate() {
            let error = errors.iter().find(|e| e.rule == i).map(|e| SharedString::from(e.message.clone()));
            let title = if rule.pattern.is_empty() { SharedString::from("(empty pattern)") } else { SharedString::from(rule.pattern.clone()) };
            frame = frame.child(
                EditableRuleRow::new(i, styler.rule_color(i), title, rule.summary())
                    .enabled(rule.enabled)
                    .selected(self.selected_rule == Some(i))
                    .error(error)
                    .can_move(i > 0, i + 1 < count)
                    .on_select(ed(&this, move |e, window, cx| {
                        let next = if e.selected_rule == Some(i) { None } else { Some(i) };
                        e.select_rule(next, window, cx);
                    }))
                    .on_toggle(ed(&this, move |e, _, cx| e.toggle_rule(i, cx)))
                    .on_move({
                        let this = this.clone();
                        move |up, _, cx| this.update(cx, |e, cx| e.move_rule(i, up, cx))
                    }),
            );
            if self.selected_rule == Some(i) {
                frame = frame.child(self.rule_form(i, rule, &this, cx));
            }
        }
        frame = frame.child(div().flex().pt(px(10.)).child(ActionButton::new("rule-add", "Add rule").on_click(ed(&this, |e, window, cx| e.add_rule(window, cx)))));
        frame = frame.child(SectionLabel::new("Filter-match palette"));
        let slots = div().flex().flex_wrap().gap(px(6.)).children(styler.hit_colors().iter().enumerate().map(|(i, h)| {
            PaletteSlot::new(i, h.fg, h.bg).selected(self.selected_slot == Some(i)).on_click(ed(&this, move |e, window, cx| {
                let next = if e.selected_slot == Some(i) { None } else { Some(i) };
                e.select_slot(next, window, cx);
            }))
        }));
        frame = frame.child(slots);
        if let Some(slot) = self.selected_slot.filter(|s| *s < profile.hit_palette.len()) {
            frame = frame.child(self.slot_form(slot, &profile, &this, cx));
        }
        frame.note("Profiles are saved in ~/Library/Application Support/rogg/profiles.")
    }
}

/// Extension points for Task 14 (the rule form, the palette slot form, import and export). They are intentionally
/// empty here: the forms render nothing and import and export do nothing until Task 14 fills them in.
impl ProfileEditor {
    fn rule_form(&self, _i: usize, _rule: &LineRule, _this: &Entity<Self>, _cx: &App) -> gpui_kit::AnyElement {
        div().into_any_element()
    }

    fn slot_form(&self, _slot: usize, _profile: &Profile, _this: &Entity<Self>, _cx: &App) -> gpui_kit::AnyElement {
        div().into_any_element()
    }

    /// Task 14 opens the file panel here.
    pub fn import(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    /// Task 14 opens the save panel here.
    pub fn export(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}
}
