//! The highlighting profile editor (spec §6.1): choose, create, duplicate, rename, delete, import and export profiles;
//! reorder, enable, add, edit and delete rules; edit the filter-match palette. Every change is saved at once and open
//! tabs restyle (the workspace listens for `Changed`).

use crate::profiles::{ProfileStore, RowStyler};
use crate::tab::Tab;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::{App, BorrowAppContext, Context, Entity, EventEmitter, SharedString, Subscription, TestSupportExt, WeakEntity, Window, div, px};
use rogg_core::profile::{
    ColorSpec, ColorValue, DEFAULT_PROFILE, EXPORT_EXT, HitEntry, LineRule, MAX_PALETTE, Panes, Profile, default_palette, derive_light, names_collide,
};
use rogg_ui::controls::{ActionButton, CompactSelect, FlagButton, Segmented};
use rogg_ui::dialogs::DialogFrame;
use rogg_ui::form::{EditableRuleRow, PaletteSlot, SectionLabel, Swatch, TextField};
use rogg_ui::theme::ActiveRoggTheme;
use std::path::Path;

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
    /// Pattern, colour and light-colour inputs of the selected rule, with their subscriptions.
    rule_inputs: Option<([Entity<InputState>; 3], Vec<Subscription>)>,
    /// Foreground, background and their light overrides of the selected palette slot.
    slot_inputs: Option<([Entity<InputState>; 4], Vec<Subscription>)>,
    color_error: bool,
}

impl EventEmitter<ProfileEditorEvent> for ProfileEditor {}

/// A callback that runs `f` on the editor.
fn ed(this: &Entity<ProfileEditor>, f: impl Fn(&mut ProfileEditor, &mut Window, &mut Context<ProfileEditor>) + 'static) -> impl Fn(&mut Window, &mut App) + 'static {
    let this = this.clone();
    move |window, cx| this.update(cx, |e, cx| f(e, window, cx))
}

impl ProfileEditor {
    pub fn new(tab: Option<WeakEntity<Tab>>, profile: String) -> Self {
        Self { tab, profile, selected_rule: None, selected_slot: None, rename: None, rename_error: None, rule_inputs: None, slot_inputs: None, color_error: false }
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
        self.rule_inputs = None;
        self.slot_inputs = None;
        self.color_error = false;
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

    /// Selects rule `i` and opens its editing form below it (with fresh inputs showing its values).
    pub fn select_rule(&mut self, i: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_rule = i;
        self.color_error = false;
        self.rule_inputs = i.and_then(|i| cx.global::<ProfileStore>().get(&self.profile).rules.get(i).cloned()).map(|rule| {
            let pattern = cx.new(|cx| InputState::new(window, cx).default_value(rule.pattern.clone()).placeholder("Text or pattern"));
            let color = cx.new(|cx| InputState::new(window, cx).default_value(rule.color.0.clone()).placeholder("#rrggbb or token"));
            let light = cx.new(|cx| InputState::new(window, cx).default_value(rule.light.map(|l| l.0).unwrap_or_default()).placeholder("auto"));
            // The subscriptions read `selected_rule` at event time: a reorder moves the selection without new inputs.
            let subs = vec![
                cx.subscribe_in(&pattern, window, |e, input, ev: &InputEvent, _, cx| {
                    if let (InputEvent::Change, Some(i)) = (ev, e.selected_rule) {
                        let text = input.read(cx).value().to_string();
                        e.edit(cx, |p| {
                            if let Some(r) = p.rules.get_mut(i) {
                                r.pattern = text;
                            }
                        });
                    }
                }),
                cx.subscribe_in(&color, window, |e, input, ev: &InputEvent, _, cx| {
                    if let (InputEvent::Change, Some(i)) = (ev, e.selected_rule) {
                        let spec = ColorSpec(input.read(cx).value().trim().to_string());
                        e.color_error = spec.parse().is_none();
                        if !e.color_error {
                            e.edit(cx, |p| {
                                if let Some(r) = p.rules.get_mut(i) {
                                    r.color = spec;
                                }
                            });
                        }
                        cx.notify();
                    }
                }),
                cx.subscribe_in(&light, window, |e, input, ev: &InputEvent, _, cx| {
                    if let (InputEvent::Change, Some(i)) = (ev, e.selected_rule) {
                        let text = input.read(cx).value().trim().to_string();
                        let spec = (!text.is_empty()).then_some(ColorSpec(text));
                        if spec.as_ref().is_none_or(|s| s.parse().is_some()) {
                            e.edit(cx, |p| {
                                if let Some(r) = p.rules.get_mut(i) {
                                    r.light = spec;
                                }
                            });
                        }
                    }
                }),
            ];
            ([pattern, color, light], subs)
        });
        cx.notify();
    }

    /// Selects palette slot `i` and opens its colour form (with fresh inputs showing its values).
    pub fn select_slot(&mut self, i: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_slot = i;
        self.slot_inputs = i.and_then(|i| cx.global::<ProfileStore>().get(&self.profile).hit_palette.get(i).cloned()).map(|h| {
            let make = |text: String, placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| InputState::new(window, cx).default_value(text).placeholder(placeholder))
            };
            let inputs = [
                make(h.fg.0.clone(), "text colour", window, cx),
                make(h.bg.0.clone(), "background", window, cx),
                make(h.light_fg.map(|c| c.0).unwrap_or_default(), "auto", window, cx),
                make(h.light_bg.map(|c| c.0).unwrap_or_default(), "auto", window, cx),
            ];
            let subs = inputs
                .iter()
                .enumerate()
                .map(|(field, input)| {
                    cx.subscribe_in(input, window, move |e, input, ev: &InputEvent, _, cx| {
                        let (InputEvent::Change, Some(slot)) = (ev, e.selected_slot) else { return };
                        let text = input.read(cx).value().trim().to_string();
                        let spec = ColorSpec(text.clone());
                        let optional = field >= 2;
                        if !(spec.parse().is_some() || (optional && text.is_empty())) {
                            return;
                        }
                        e.edit(cx, |p| {
                            let Some(h) = p.hit_palette.get_mut(slot) else { return };
                            match field {
                                0 => h.fg = spec,
                                1 => h.bg = spec,
                                2 => h.light_fg = (!text.is_empty()).then_some(spec),
                                _ => h.light_bg = (!text.is_empty()).then_some(spec),
                            }
                        });
                    })
                })
                .collect();
            (inputs, subs)
        });
        cx.notify();
    }

    pub fn rule_inputs(&self) -> Option<(Entity<InputState>, Entity<InputState>, Entity<InputState>)> {
        self.rule_inputs.as_ref().map(|([p, c, l], _)| (p.clone(), c.clone(), l.clone()))
    }

    pub fn slot_inputs(&self) -> Option<[Entity<InputState>; 4]> {
        self.slot_inputs.as_ref().map(|(inputs, _)| inputs.clone())
    }

    /// Whether the rule's colour field holds something that is neither a token nor a hex colour.
    pub fn color_error(&self) -> bool {
        self.color_error
    }

    /// A swatch click: the token becomes the rule colour (its light override is dropped) and the colour field shows it.
    fn set_token(&mut self, i: usize, token: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(cx, |p| {
            if let Some(r) = p.rules.get_mut(i) {
                r.color = ColorSpec::token(token);
                r.light = None;
            }
        });
        self.color_error = false;
        if let Some(([_, color, light], _)) = &self.rule_inputs {
            color.update(cx, |c, cx| c.set_value(token, window, cx));
            light.update(cx, |l, cx| l.set_value("", window, cx));
        }
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

const SWATCH_TOKENS: [&str; 10] = ["text", "muted", "faint", "accent", "error", "amber", "hit-0", "hit-1", "hit-2", "hit-3"];

/// A labelled line of the rule and slot forms.
fn form_line(label: &'static str, cx: &App) -> gpui_kit::Div {
    let s = cx.rogg().text_scale();
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().w(px(74.)).flex_none().text_size(s.px(11.)).text_color(cx.rogg().palette.muted).child(label))
}

impl ProfileEditor {
    fn rule_form(&self, i: usize, rule: &LineRule, this: &Entity<Self>, cx: &App) -> gpui_kit::AnyElement {
        let Some(([pattern, color, light], _)) = &self.rule_inputs else { return div().into_any_element() };
        let p = cx.rogg().palette;
        let has_error = cx.global::<ProfileStore>().errors(&self.profile).iter().any(|e| e.rule == i);
        let flag = |id: &'static str, label: &'static str, on: bool, f: fn(&mut LineRule)| {
            FlagButton::new(id, label).pressed(on).on_click(ed(this, move |e, _, cx| {
                e.edit(cx, |p| {
                    if let Some(r) = p.rules.get_mut(i) {
                        f(r)
                    }
                })
            }))
        };
        let derived = match rule.color.parse() {
            Some(ColorValue::Rgba(v)) => format!("auto · {}", ColorSpec::hex(derive_light(v, false)).0),
            _ => "follows the theme".to_string(),
        };
        let panes = [Panes::Full, Panes::Filtered, Panes::Both];
        div()
            .id("rule-form")
            .test_support()
            .flex()
            .flex_col()
            .gap(px(8.))
            .pl(px(37.))
            .pr(px(4.))
            .pb(px(12.))
            .child(
                form_line("Pattern", cx)
                    .child(TextField::new("rule-pattern", pattern.clone()).mono(true).width(240.).error(has_error))
                    .child(flag("rule-regex", ".*", rule.regex, |r| r.regex = !r.regex))
                    .child(flag("rule-case", "Aa", rule.case_sensitive, |r| r.case_sensitive = !r.case_sensitive)),
            )
            .child(
                form_line("Effects", cx)
                    .child(flag("rule-effect-match", "Match text", rule.match_color, |r| r.match_color = !r.match_color))
                    .child(flag("rule-effect-line", "Line text", rule.line_color, |r| r.line_color = !r.line_color))
                    .child(flag("rule-effect-tint", "Row tint", rule.row_tint, |r| r.row_tint = !r.row_tint))
                    .child(flag("rule-effect-bold", "Bold", rule.bold, |r| r.bold = !r.bold)),
            )
            .child(form_line("Panels", cx).child(
                Segmented::new("rule-panes", vec!["Full log".into(), "Filtered log".into(), "Both".into()], panes.iter().position(|x| *x == rule.panes).unwrap_or(2)).on_select({
                    let this = this.clone();
                    move |k, _, cx| {
                        this.update(cx, |e, cx| {
                            e.edit(cx, |p| {
                                if let Some(r) = p.rules.get_mut(i) {
                                    r.panes = panes[k.min(2)];
                                }
                            })
                        })
                    }
                }),
            ))
            .child(
                form_line("Colour", cx)
                    .children(SWATCH_TOKENS.iter().map(|token| {
                        let color = crate::profiles::resolve_color(&ColorSpec::token(token), None, false, &p, cx.rogg().appearance);
                        Swatch::new(SharedString::from(format!("rule-swatch-{token}")), color)
                            .selected(rule.color == ColorSpec::token(token))
                            .on_click(ed(this, move |e, window, cx| e.set_token(i, token, window, cx)))
                    }))
                    .child(TextField::new("rule-color", color.clone()).mono(true).width(96.).error(self.color_error)),
            )
            .child(
                form_line("Light theme", cx)
                    .child(TextField::new("rule-light", light.clone()).mono(true).width(96.))
                    .child(div().text_size(cx.rogg().text_scale().px(10.)).text_color(p.faint).child(derived)),
            )
            .child(div().flex().child(ActionButton::new("rule-delete", "Delete rule").on_click(ed(this, move |e, window, cx| e.delete_rule(i, window, cx)))))
            .into_any_element()
    }

    fn slot_form(&self, slot: usize, profile: &Profile, this: &Entity<Self>, cx: &App) -> gpui_kit::AnyElement {
        let Some(([fg, bg, light_fg, light_bg], _)) = &self.slot_inputs else { return div().into_any_element() };
        let len = profile.hit_palette.len();
        div()
            .id("slot-form")
            .test_support()
            .flex()
            .flex_col()
            .gap(px(8.))
            .pt(px(10.))
            .child(form_line("Colours", cx).child(TextField::new("slot-fg", fg.clone()).mono(true).width(96.)).child(TextField::new("slot-bg", bg.clone()).mono(true).width(96.)))
            .child(
                form_line("Light theme", cx)
                    .child(TextField::new("slot-light-fg", light_fg.clone()).mono(true).width(96.))
                    .child(TextField::new("slot-light-bg", light_bg.clone()).mono(true).width(96.)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .when(len < MAX_PALETTE, |d| {
                        d.child(ActionButton::new("slot-add", "Add colour").on_click(ed(this, move |e, window, cx| {
                            e.edit(cx, |p| p.hit_palette.push(HitEntry::slot(p.hit_palette.len())));
                            let last = cx.global::<ProfileStore>().get(&e.profile).hit_palette.len() - 1;
                            e.select_slot(Some(last), window, cx);
                        })))
                    })
                    .when(len > 1, |d| {
                        d.child(ActionButton::new("slot-remove", "Remove colour").on_click(ed(this, move |e, window, cx| {
                            e.edit(cx, |p| {
                                if slot < p.hit_palette.len() && p.hit_palette.len() > 1 {
                                    p.hit_palette.remove(slot);
                                }
                            });
                            e.select_slot(None, window, cx);
                        })))
                    }),
            )
            .into_any_element()
    }

    /// Import… (spec §6.1 `.roggprofile`): a taken name gets a number; a broken file is reported, never renamed.
    pub fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui_kit::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Import".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |e, cx| e.import_profile_from(&path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn import_profile_from(&mut self, path: &Path, cx: &mut Context<Self>) {
        match rogg_core::profile::import(path) {
            Ok(profile) => {
                let name = cx.update_global::<ProfileStore, _>(|s, _| s.add(profile));
                cx.emit(ProfileEditorEvent::Changed);
                cx.emit(ProfileEditorEvent::Toast(format!("Imported profile {name}").into()));
                self.select_profile(&name, cx);
            }
            Err(e) => cx.emit(ProfileEditorEvent::Toast(format!("Could not import {}: {e}", file_label(path)).into())),
        }
    }

    /// Export…: the shown profile as `<name>.roggprofile`.
    pub fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let suggested = format!("{}.{EXPORT_EXT}", self.profile);
        let rx = cx.prompt_for_new_path(&rogg_core::paths::home_dir(), Some(&suggested));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update(cx, |e, cx| e.export_profile_to(&path, cx)).ok();
            }
        })
        .detach();
    }

    pub fn export_profile_to(&mut self, path: &Path, cx: &mut Context<Self>) {
        let profile = cx.global::<ProfileStore>().get(&self.profile).clone();
        let file = file_label(path);
        let message = match rogg_core::profile::export(path, &profile) {
            Ok(()) => format!("Exported profile to {file}"),
            Err(e) => format!("Could not export {file}: {e}"),
        };
        cx.emit(ProfileEditorEvent::Toast(message.into()));
    }
}

/// The file name shown in toasts.
fn file_label(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}
