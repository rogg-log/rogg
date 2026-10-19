//! Self-update in the window (spec §3): the startup check, polling the updater's job, the title-bar badge, the
//! Update dialog and its buttons.

use super::{Dialog, Workspace, act};
use crate::document::POLL_INTERVAL;
use crate::settings::SettingsStore;
use crate::updater::view::{self, INSTALL_COMMAND, ViewActions};
use crate::updater::{UpdateState, Updater};
use gpui_kit::{App, BorrowAppContext, ClipboardItem, Context, Entity, Window};
use rogg_ui::Callback;
use rogg_ui::dialogs::{self, DialogFrame, UpdateView};
use rogg_ui::titlebar::TitleBar;
use std::rc::Rc;
use std::time::Duration;

/// The startup check runs this long after the window opens (spec §3.1).
pub const STARTUP_CHECK_DELAY: Duration = Duration::from_secs(3);

impl Workspace {
    /// After `STARTUP_CHECK_DELAY`, a silent check when the setting is on and nothing has happened yet (a manual
    /// check in the meantime has already said where things stand).
    pub(super) fn schedule_startup_check(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(STARTUP_CHECK_DELAY).await;
            this.update_in(cx, |ws, window, cx| {
                let idle = cx.try_global::<Updater>().is_some_and(|u| matches!(u.state(), UpdateState::Idle));
                if idle && SettingsStore::get(cx).behavior.check_updates {
                    ws.check_for_updates(false, window, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Fetches the manifest. A `manual` check (Check for Updates…) opens the Update dialog on the outcome, or on the
    /// running job when there is one (spec §3.4).
    pub fn check_for_updates(&mut self, manual: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !cx.has_global::<Updater>() {
            return;
        }
        cx.update_global::<Updater, _>(|u, _| u.check(manual));
        if manual {
            self.open_dialog(Dialog::Update, cx);
        }
        self.ensure_update_polling(window, cx);
        cx.notify();
    }

    /// Update: downloads and installs the release on offer.
    fn start_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cx.has_global::<Updater>() {
            cx.update_global::<Updater, _>(|u, _| u.start_update());
            self.ensure_update_polling(window, cx);
        }
        cx.notify();
    }

    /// Cancel: the state returns to the offer once the job has cleaned up (the polling picks that up).
    fn cancel_update(&mut self, cx: &mut Context<Self>) {
        if cx.has_global::<Updater>() {
            cx.update_global::<Updater, _>(|u, _| u.cancel());
        }
        cx.notify();
    }

    /// Retry re-runs what failed: the update when it was one, else the check.
    fn retry_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let update = cx.try_global::<Updater>().is_some_and(|u| matches!(u.state(), UpdateState::Failed { release: Some(_), .. }));
        if update {
            self.start_update(window, cx);
        } else {
            self.check_for_updates(true, window, cx);
        }
    }

    /// Release notes: the release's GitHub page.
    fn open_release_notes(&mut self, cx: &mut Context<Self>) {
        let url = cx.try_global::<Updater>().and_then(|u| view::release_of(u.state()).map(|r| view::release_page(&r.manifest)));
        if let Some(url) = url {
            self.open_url(url, cx);
        }
    }

    fn copy_install_command(&mut self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(INSTALL_COMMAND.to_string()));
        self.toast("Install command copied", cx);
    }

    /// Opens `url` in the browser, and remembers it for tests.
    pub fn open_url(&mut self, url: String, cx: &mut Context<Self>) {
        if self.open_urls {
            cx.open_url(&url);
        }
        self.last_opened_url = Some(url);
    }

    pub fn last_opened_url(&self) -> Option<&str> {
        self.last_opened_url.as_deref()
    }

    /// One loop applies the job's reports and re-renders while a job runs; it ends with the job.
    fn ensure_update_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_polling {
            return;
        }
        self.update_polling = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let running = this.update(cx, |ws, cx| {
                    let running = cx.has_global::<Updater>() && cx.update_global::<Updater, _>(|u, _| u.poll());
                    ws.update_polling = running;
                    cx.notify();
                    running
                });
                if !matches!(running, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    /// The title bar with the update badge, when the updater has one; it opens the Update dialog.
    pub(super) fn with_update_badge(&self, bar: TitleBar, this: &Entity<Self>, cx: &App) -> TitleBar {
        match cx.try_global::<Updater>().and_then(Updater::badge) {
            Some((label, tooltip)) => bar.update_badge(label, tooltip, act(this, |ws, _, cx| ws.open_dialog(Dialog::Update, cx))),
            None => bar,
        }
    }

    pub(super) fn update_dialog(&self, this: &Entity<Self>, cx: &App) -> DialogFrame {
        let callback = |f: fn(&mut Workspace, &mut Window, &mut Context<Workspace>)| -> Callback { Rc::new(act(this, f)) };
        let actions = ViewActions {
            update: callback(|ws, window, cx| ws.start_update(window, cx)),
            cancel: callback(|ws, _, cx| ws.cancel_update(cx)),
            retry: callback(|ws, window, cx| ws.retry_update(window, cx)),
            notes: callback(|ws, _, cx| ws.open_release_notes(cx)),
            copy_command: callback(|ws, _, cx| ws.copy_install_command(cx)),
            close: callback(|ws, _, cx| ws.close_dialog(cx)),
        };
        dialogs::update(current_view(actions, cx))
    }

    /// The open Update dialog's lines (then its progress label) and button ids, for tests; None when it isn't open.
    pub fn update_dialog_content(&self, cx: &App) -> Option<(Vec<String>, Vec<&'static str>)> {
        if !matches!(self.dialog, Some(Dialog::Update)) {
            return None;
        }
        let none: Callback = Rc::new(|_, _| {});
        let actions = ViewActions {
            update: none.clone(),
            cancel: none.clone(),
            retry: none.clone(),
            notes: none.clone(),
            copy_command: none.clone(),
            close: none,
        };
        let view = current_view(actions, cx);
        let mut lines: Vec<String> = view.lines.iter().map(|l| l.to_string()).collect();
        lines.extend(view.progress.map(|(_, label)| label.to_string()));
        Some((lines, view.buttons.iter().map(|b| b.id).collect()))
    }
}

fn current_view(actions: ViewActions, cx: &App) -> UpdateView {
    match cx.try_global::<Updater>() {
        Some(u) => view::update_view(u.state(), u.config().running_build, actions),
        None => view::update_view(&UpdateState::Idle, None, actions),
    }
}
