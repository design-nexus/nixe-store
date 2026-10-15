//! Store discovery uses the suite's frame and palette, with editorial app-store content.
use super::*;

pub(super) fn render(catalog: &Catalog) {
    let mut seen = BTreeSet::new();
    let apps: Vec<App> = catalog
        .apps
        .iter()
        .filter(|a| a.desktop && repositories::official(&a.source) && source_match(a))
        .filter(|a| seen.insert(a.package.clone()))
        .cloned()
        .collect();
    if apps.is_empty() {
        text("Select All sources or Arch to discover desktop apps.");
        return;
    }
    featured(&apps);
    collections(&apps);
    heading("Everyday essentials", "A few great places to start. Handpicked from Arch.");
    let picks: Vec<App> = ["firefox", "libreoffice-fresh", "keepassxc", "vlc", "signal-desktop", "obsidian"]
        .iter()
        .filter_map(|name| apps.iter().find(|a| a.package == *name).cloned())
        .collect();
    grid(&picks);
    for (category, title, copy) in [
        ("Graphics", "Make something extraordinary", "From the first sketch to the final frame."),
        ("Audio & video", "Your own creative studio", "Record it. Cut it. Share it with the world."),
        ("Games", "Clock out. Play on.", "A little adventure belongs on every desktop."),
        ("Development", "Build what comes next", "Great tools for your next big idea."),
    ] {
        let mut selection: Vec<App> = apps.iter().filter(|a| a.category == category).cloned().collect();
        selection.sort_by_key(|a| !a.popular);
        selection.truncate(6);
        if selection.is_empty() {
            continue;
        }
        let header = widgets::hbox(8);
        header.add_css_class("home-collection-heading");
        let title = widgets::label(title, "home-section-title");
        title.set_wrap(true);
        title.set_xalign(0.0);
        title.set_hexpand(true);
        header.append(&title);
        let more = button("See all →", move || category_page(category));
        more.add_css_class("bar-button");
        header.append(&more);
        ui().body.append(&header);
        let note = widgets::label(copy, "collection-note");
        note.set_xalign(0.0);
        note.set_wrap(true);
        ui().body.append(&note);
        grid(&selection);
    }
}

fn category_page(category: &str) {
    let u = ui();
    *u.page.borrow_mut() = "categories".into();
    *u.category.borrow_mut() = category.into();
    prefs::update(|p| p.last_section = "categories".into());
    u.scroll.vadjustment().set_value(0.0);
    super::render();
}

fn featured(apps: &[App]) {
    let scenes: Vec<(&App, &str, &str)> = [
        ("blender", "MAKE SOMETHING BIG", "Bring your imagination into another dimension."),
        ("krita", "A CANVAS WITHOUT LIMITS", "A whole studio of brushes. One blank canvas. Your next idea."),
        ("obs-studio", "READY FOR YOUR CLOSE-UP", "Turn your screen into a studio, and your ideas into a broadcast."),
    ]
    .into_iter()
    .filter_map(|(package, title, copy)| apps.iter().find(|a| a.package == package).map(|a| (a, title, copy)))
    .collect();
    if scenes.is_empty() {
        return;
    }
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(if prefs::get().reduce_motion { 0 } else { 160 });
    for (index, (app, title, copy)) in scenes.iter().enumerate() {
        let tile = gtk::Button::new();
        tile.add_css_class("feature-hero");
        tile.set_tooltip_text(Some(&format!("Explore {}", app.name)));
        let overlay = gtk::Overlay::new();
        overlay.set_overflow(gtk::Overflow::Hidden);
        let row = widgets::hbox(12);
        row.add_css_class("feature-content");
        let words = widgets::vbox(10);
        words.set_hexpand(true);
        words.set_valign(gtk::Align::Center);
        let eyebrow = widgets::label(title, "feature-eyebrow");
        eyebrow.set_xalign(0.0);
        eyebrow.set_wrap(true);
        words.append(&eyebrow);
        let name = widgets::label(&app.name, "feature-title");
        name.set_xalign(0.0);
        name.set_wrap(true);
        words.append(&name);
        let description = widgets::label(copy, "feature-description");
        description.set_xalign(0.0);
        description.set_wrap(true);
        description.set_max_width_chars(36);
        words.append(&description);
        let action = widgets::label(&format!("Explore {}  →", app.name), "feature-action");
        action.set_halign(gtk::Align::Start);
        words.append(&action);
        let tag = widgets::label(
            &format!("{}{}", app.source, if app.installed.is_some() { " · In your library" } else { " · Available on Arch" }),
            "source-tag",
        );
        tag.set_xalign(0.0);
        words.append(&tag);
        row.append(&words);
        let art = app_icon(app);
        art.set_pixel_size(100);
        art.add_css_class("feature-app-icon");
        art.set_valign(gtk::Align::Center);
        row.append(&art);
        let backdrop = gtk::Picture::new();
        backdrop.set_can_shrink(true);
        backdrop.set_content_fit(gtk::ContentFit::Cover);
        let base = widgets::hbox(0);
        overlay.set_child(Some(&base));
        overlay.add_overlay(&backdrop);
        // Overlays never contribute the screenshot's natural width to the window.
        let shade = widgets::hbox(0);
        shade.add_css_class("feature-shade");
        overlay.add_overlay(&shade);
        overlay.add_overlay(&row);
        overlay.set_measure_overlay(&row, true);
        load_picture(app, 0, &backdrop);
        tile.set_child(Some(&overlay));
        let app = (*app).clone();
        tile.connect_clicked(move |_| details(app.clone()));
        stack.add_named(&tile, Some(&index.to_string()));
    }
    ui().body.append(&stack);
    let controls = widgets::hbox(8);
    controls.add_css_class("feature-controls");
    let label = widgets::label("THE SPOTLIGHT", "source-tag");
    label.set_xalign(0.0);
    label.set_hexpand(true);
    controls.append(&label);
    let count = scenes.len();
    let current = Rc::new(Cell::new(0usize));
    let position = widgets::label(&format!("01 / {count:02}"), "source-tag");
    for (icon, tip, forwards) in
        [("go-previous-symbolic", "Previous featured app", false), ("go-next-symbolic", "Next featured app", true)]
    {
        let arrow = widgets::bar_button(icon, tip);
        let (stack, current, position) = (stack.clone(), current.clone(), position.clone());
        arrow.connect_clicked(move |_| {
            let index = if forwards { (current.get() + 1) % count } else { (current.get() + count - 1) % count };
            current.set(index);
            stack.set_visible_child_name(&index.to_string());
            position.set_text(&format!("{:02} / {count:02}", index + 1));
        });
        controls.append(&arrow);
    }
    controls.append(&position);
    ui().body.append(&controls);
}

fn collections(apps: &[App]) {
    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .min_children_per_line(2)
        .max_children_per_line(4)
        .column_spacing(10)
        .row_spacing(10)
        .homogeneous(true)
        .build();
    grid.add_css_class("home-collections");
    for (title, category, icon, class) in [
        ("Create", "Graphics", "applications-graphics-symbolic", "collection-create"),
        ("Work", "Productivity", "accessories-text-editor-symbolic", "collection-work"),
        ("Play", "Games", "applications-games-symbolic", "collection-play"),
        ("Develop", "Development", "utilities-terminal-symbolic", "collection-develop"),
    ] {
        let tile = button(title, move || category_page(category));
        tile.add_css_class("collection-tile");
        tile.add_css_class(class);
        let content = widgets::vbox(8);
        let icon = gtk::Image::from_icon_name(icon);
        icon.set_pixel_size(28);
        content.append(&icon);
        content.append(&widgets::label(title, "app-name"));
        let count = apps.iter().filter(|a| a.category == category).count();
        content.append(&widgets::label(&format!("{count} apps"), "source-tag"));
        tile.set_child(Some(&content));
        grid.insert(&tile, -1);
    }
    ui().body.append(&grid);
}
