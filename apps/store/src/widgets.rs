//! Nyx-style building blocks: pages, groups and option rows.

pub use nyx_ui::widgets::*;

use gtk::prelude::*;

// ---------- Page / group ----------

pub struct Page {
    pub root: gtk::ScrolledWindow,
    pub body: gtk::Box,
}

/// A page of the main window. Its name shows in the top bar, so the page itself
/// starts straight with its content.
pub fn page(section: &str) -> Page {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.add_css_class("settings-page");
    body.add_css_class(&format!("page-{section}"));
    body.set_hexpand(true);

    let root = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        // Scrolls with wheel, trackpad and keyboard; no visible scrollbar.
        .vscrollbar_policy(gtk::PolicyType::External)
        .child(&body)
        .vexpand(true)
        .build();
    Page { root, body }
}
