//! Setup uses the suite's modal card, with persistent progress and a retry path.
use super::*;

struct Setup {
    window: gtk::Window,
    status: gtk::Label,
    spinner: gtk::Spinner,
    progress: gtk::ProgressBar,
    steps: Vec<(gtk::Image, gtk::Label)>,
    log: gtk::TextBuffer,
    close: gtk::Button,
    retry: gtk::Button,
    running: Cell<bool>,
    stage: Cell<usize>,
}
thread_local! { static SETUP: RefCell<Option<Rc<Setup>>> = const { RefCell::new(None) }; }
const STEPS: &[&str] = &[
    "Authorize and check installed support",
    "Download repository support",
    "Verify BlackArch signatures",
    "Install the signing keyring",
    "Configure signing keys",
    "Enable the repository",
];

pub(super) fn open() {
    let (window, card) = widgets::dialog("Set up BlackArch", 560);
    let description = widgets::label(
        "Enter your password in the authorization prompt. Store will set up BlackArch automatically, keeping Omarchy first and preserving installed app sources.",
        "dim",
    );
    description.set_wrap(true);
    description.set_max_width_chars(62);
    card.append(&description);
    let list = widgets::vbox(0);
    list.add_css_class("group-list");
    let steps = STEPS
        .iter()
        .map(|title| {
            let icon = gtk::Image::from_icon_name("content-loading-symbolic");
            let row = widgets::hbox(12);
            row.add_css_class("settings-option");
            row.append(&icon);
            let label = widgets::label(title, "dim");
            label.set_hexpand(true);
            label.set_wrap(true);
            row.append(&label);
            list.append(&row);
            (icon, label)
        })
        .collect();
    card.append(&list);
    let activity = widgets::hbox(10);
    let spinner = gtk::Spinner::new();
    spinner.start();
    activity.append(&spinner);
    let status = widgets::label("Waiting for password authorization…", "");
    status.set_wrap(true);
    status.set_hexpand(true);
    status.set_max_width_chars(62);
    activity.append(&status);
    card.append(&activity);
    let progress = gtk::ProgressBar::new();
    card.append(&progress);
    let log = gtk::TextBuffer::new(None::<&gtk::TextTagTable>);
    let view = gtk::TextView::builder()
        .buffer(&log)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&view)
        .min_content_height(110)
        .max_content_height(160)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    log.connect_changed(glib::clone!(
        #[weak]
        view,
        move |buffer| {
            view.scroll_to_iter(&mut buffer.end_iter(), 0.0, false, 0.0, 1.0);
        }
    ));
    let details = gtk::Expander::builder().label("Setup details").child(&scroll).build();
    card.append(&details);
    let actions = widgets::hbox(8);
    actions.set_halign(gtk::Align::End);
    let close = gtk::Button::with_label("Close");
    close.set_sensitive(false);
    close.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| window.close()
    ));
    let retry = gtk::Button::with_label("Retry setup");
    retry.add_css_class("suggested-action");
    retry.set_visible(false);
    retry.connect_clicked(glib::clone!(
        #[weak]
        window,
        move |_| {
            window.close();
            start_request(Request::BlackarchSetup);
        }
    ));
    actions.append(&close);
    actions.append(&retry);
    card.append(&actions);
    let setup = Rc::new(Setup {
        window,
        status,
        spinner,
        progress,
        steps,
        log,
        close,
        retry,
        running: Cell::new(true),
        stage: Cell::new(0),
    });
    setup.window.connect_close_request(glib::clone!(
        #[weak]
        setup,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_| if setup.running.get() { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    ));
    glib::timeout_add_local(
        std::time::Duration::from_millis(150),
        glib::clone!(
            #[weak]
            setup,
            #[upgrade_or]
            glib::ControlFlow::Break,
            move || {
                if !setup.running.get() {
                    return glib::ControlFlow::Break;
                }
                setup.progress.pulse();
                glib::ControlFlow::Continue
            }
        ),
    );
    SETUP.with(|slot| *slot.borrow_mut() = Some(setup.clone()));
    update("Waiting for password authorization…");
    setup.window.present();
}

pub(super) fn update(message: &str) {
    SETUP.with(|slot| {
        let slot = slot.borrow();
        let Some(s) = slot.as_ref() else { return };
        s.status.set_text(message.trim());
        s.log.insert(&mut s.log.end_iter(), &format!("{}\n", message.trim()));
        let stage = if message.starts_with("Downloading") {
            1
        } else if message.starts_with("Verifying") {
            2
        } else if message.starts_with("Installing") {
            3
        } else if message.starts_with("Configuring") {
            4
        } else if message.starts_with("Enabling") {
            5
        } else {
            s.stage.get()
        };
        if message.starts_with("Using the installed") {
            s.steps[1].1.set_text("Download not needed — keyring installed");
            s.steps[2].1.set_text("Use existing signing support");
            s.steps[3].1.set_text("Keyring already installed");
        }
        s.stage.set(stage);
        for (i, (icon, label)) in s.steps.iter().enumerate() {
            icon.set_icon_name(Some(if i < stage {
                "emblem-ok-symbolic"
            } else if i == stage {
                "media-playback-start-symbolic"
            } else {
                "content-loading-symbolic"
            }));
            if i == stage {
                label.remove_css_class("dim");
            } else {
                label.add_css_class("dim");
            }
        }
    });
}

pub(super) fn finish(message: &str, success: bool) {
    update(message);
    SETUP.with(|slot| {
        let slot = slot.borrow();
        let Some(s) = slot.as_ref() else { return };
        s.running.set(false);
        s.spinner.stop();
        s.spinner.set_visible(false);
        s.progress.set_fraction(if success { 1.0 } else { 0.0 });
        s.progress.set_visible(success);
        s.close.set_sensitive(true);
        s.retry.set_visible(!success);
        if success {
            for (icon, label) in &s.steps {
                icon.set_icon_name(Some("emblem-ok-symbolic"));
                label.remove_css_class("dim");
            }
            s.status.set_text("BlackArch is ready. The app catalog is refreshing; you can close this window.");
        } else {
            s.steps[s.stage.get()].0.set_icon_name(Some("dialog-error-symbolic"));
        }
        s.window.present();
    });
}
