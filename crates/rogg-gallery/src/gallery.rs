use crate::{fixtures, pages};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Context, Entity, FontWeight, SharedString, Window, div, px, relative};
use rogg_core::settings::ThemeSetting;
use rogg_ui::controls::{FindField, Segmented, TextButton};
use rogg_ui::logview::{LogViewState, VecRows};
use rogg_ui::theme::{ActiveRoggTheme, Appearance, Density, RoggTheme, size};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Page {
    Tokens,
    Controls,
    Tabs,
    Sidebar,
    Filter,
    LogView,
    Chrome,
    Empty,
    Dialogs,
    Replica,
    App,
}

impl Page {
    pub const ALL: [Page; 11] = [
        Page::Tokens,
        Page::Controls,
        Page::Tabs,
        Page::Sidebar,
        Page::Filter,
        Page::LogView,
        Page::Chrome,
        Page::Empty,
        Page::Dialogs,
        Page::Replica,
        Page::App,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Page::Tokens => "tokens",
            Page::Controls => "controls",
            Page::Tabs => "tabs",
            Page::Sidebar => "sidebar",
            Page::Filter => "filter",
            Page::LogView => "logview",
            Page::Chrome => "chrome",
            Page::Empty => "empty",
            Page::Dialogs => "dialogs",
            Page::Replica => "replica",
            Page::App => "app",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Tokens => "Tokens",
            Page::Controls => "Controls",
            Page::Tabs => "Title bar & tabs",
            Page::Sidebar => "Sidebar",
            Page::Filter => "Filter band",
            Page::LogView => "Log view",
            Page::Chrome => "Info, pane & status bars",
            Page::Empty => "Empty states",
            Page::Dialogs => "Dialogs",
            Page::Replica => "Mockup replica",
            Page::App => "Real app (demo)",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.slug() == slug)
    }
}

pub struct Inputs {
    pub find_empty: Entity<InputState>,
    pub find_query: Entity<InputState>,
    pub expression: Entity<InputState>,
    pub goto: Entity<InputState>,
    pub form_text: Entity<InputState>,
}

pub struct GalleryView {
    pub page: Page,
    pub chrome: bool,
    pub inputs: Inputs,
    pub log_compact: Entity<LogViewState>,
    pub log_wrapped: Entity<LogViewState>,
    pub replica: Entity<crate::replica::ReplicaState>,
    pub app: Option<Entity<rogg_app::workspace::Workspace>>,
}

impl GalleryView {
    pub fn new(page: Page, chrome: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let inputs = Inputs {
            find_empty: FindField::state("Find in full log", window, cx),
            find_query: cx.new(|cx| InputState::new(window, cx).placeholder("Find in results").default_value("timeout")),
            expression: cx.new(|cx| InputState::new(window, cx).default_value("error || timeout || healthcheck || retry")),
            goto: cx.new(|cx| InputState::new(window, cx).placeholder("Line number, e.g. 120492")),
            form_text: cx.new(|cx| InputState::new(window, cx).default_value("#e0c283")),
        };
        let rows = Rc::new(VecRows::new(fixtures::log_rows()));
        let log_compact = cx.new(|_| LogViewState::new(rows.clone()));
        log_compact.update(cx, |s, cx| {
            s.set_selected(Some(120492), cx);
            s.force_hover_row(Some(1), cx);
            s.force_hbar(true, cx);
        });
        let log_wrapped = cx.new(|_| LogViewState::new(rows));
        log_wrapped.update(cx, |s, cx| s.set_wrap(true, cx));
        let replica = cx.new(|cx| crate::replica::ReplicaState::new(window, cx));
        Self { page, chrome, inputs, log_compact, log_wrapped, replica, app: (page == Page::App).then(|| Self::new_app(window, cx)) }
    }

    /// The real app window in demo mode, with drawn traffic lights (headless windows have no native ones).
    pub fn new_app(window: &mut Window, cx: &mut Context<Self>) -> Entity<rogg_app::workspace::Workspace> {
        cx.new(|cx| {
            let mut ws = rogg_app::workspace::Workspace::new(rogg_app::args::Args { demo: true, paths: vec![] }, window, cx);
            ws.set_traffic_lights(true, cx);
            // The host (gallery toolbar or --theme) owns the theme: pin the workspace to it instead of the system appearance.
            let choice = if cx.rogg().appearance == Appearance::Dark { ThemeSetting::Dark } else { ThemeSetting::Light };
            ws.pin_theme(choice, window, cx);
            ws
        })
    }

    /// The two log panels whose rows go into capture geometry: the replica's, or the app's active tab.
    pub fn log_states(&self, cx: &gpui_kit::App) -> [(&'static str, Entity<LogViewState>); 2] {
        if self.page == Page::App
            && let Some(tab) = self.app.as_ref().and_then(|a| a.read(cx).active_tab())
        {
            let t = tab.read(cx);
            return [("top", t.full.clone()), ("bottom", t.filtered.clone())];
        }
        let r = self.replica.read(cx);
        [("top", r.top.clone()), ("bottom", r.bottom.clone())]
    }
}

impl GalleryView {
    /// Page only, no gallery chrome; the replica opens `dialog` if given.
    pub fn for_capture(page: Page, dialog: Option<crate::replica::ReplicaDialog>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        use crate::replica::ReplicaDialog;
        use rogg_app::workspace::Dialog;
        let this = Self::new(page, false, window, cx);
        match (page, dialog, this.app.clone()) {
            (Page::App, Some(d), Some(app)) => {
                let dialog = match d {
                    ReplicaDialog::AllTabs => Dialog::AllTabs,
                    ReplicaDialog::Shortcuts => Dialog::Shortcuts,
                    ReplicaDialog::History => Dialog::History,
                    ReplicaDialog::Profile | ReplicaDialog::Appearance => unreachable!("rejected by CaptureOptions::parse"),
                };
                app.update(cx, |ws, cx| ws.open_dialog(dialog, cx));
            }
            (_, Some(d), _) => this.replica.update(cx, |r, cx| r.open_dialog(d, cx)),
            _ => {}
        }
        this
    }
}

impl Render for GalleryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.rogg().clone();
        let p = t.palette;
        let content = pages::render(self, window, cx);
        if !self.chrome {
            return div().size_full().bg(p.bg).text_color(p.text).font_family(t.fonts.ui.clone()).text_size(px(13.)).line_height(relative(size::LINE_HEIGHT)).child(content).into_any_element();
        }
        let view = cx.entity().downgrade();
        let nav = div()
            .w(px(190.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(10.))
            .bg(p.side)
            .border_r_1()
            .border_color(p.border)
            .child(div().px(px(6.)).pb(px(12.)).text_size(px(14.)).font_weight(FontWeight(600.)).text_color(p.muted).child(format!("rogg gallery {}", rogg_ui::version())))
            .children(Page::ALL.map(|page| {
                let view = view.clone();
                div().rounded(px(5.)).when(page == self.page, |d| d.bg(p.selection)).child(
                    TextButton::meta(SharedString::from(format!("nav-{}", page.slug())), page.title()).on_click(move |_, cx| {
                        view.update(cx, |this, cx| {
                            this.page = page;
                            cx.notify();
                        })
                        .ok();
                    }),
                )
            }));
        let appearance_ix = if t.appearance == Appearance::Dark { 0 } else { 1 };
        let density_ix = if t.density == Density::Compact { 0 } else { 1 };
        let toolbar = div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .bg(p.side)
            .border_b_1()
            .border_color(p.border)
            .child(div().text_size(px(15.)).font_weight(FontWeight(550.)).child(self.page.title()))
            .child(div().flex_1())
            .child(Segmented::new("gallery-appearance", vec!["Dark".into(), "Light".into()], appearance_ix).on_select(|i, _, cx| {
                let density = cx.rogg().density;
                RoggTheme::new(if i == 0 { Appearance::Dark } else { Appearance::Light }, density).install(cx);
            }))
            .child(Segmented::new("gallery-density", vec!["Compact".into(), "Comfortable".into()], density_ix).on_select(|i, _, cx| {
                let appearance = cx.rogg().appearance;
                RoggTheme::new(appearance, if i == 0 { Density::Compact } else { Density::Comfortable }).install(cx);
            }));
        div()
            .size_full()
            .flex()
            .bg(p.page)
            .text_color(p.text)
            .font_family(t.fonts.ui.clone())
            .text_size(px(13.))
            .line_height(relative(size::LINE_HEIGHT))
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .child(div().id("page").flex_1().min_h_0().overflow_y_scroll().p(px(24.)).child(content)),
            )
            .into_any_element()
    }
}
