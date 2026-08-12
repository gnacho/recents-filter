use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use recents_filter::config::Config;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "org.gnacho.RecentsFilter";

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let app = libadwaita::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(build_ui);
    app.run()
}

fn section_title(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_halign(gtk4::Align::Start);
    label.add_css_class("title-4");
    label.add_css_class("caption-heading");
    label.set_margin_top(12);
    label.set_margin_start(6);
    label.set_margin_bottom(4);
    label
}

fn build_ui(app: &libadwaita::Application) {
    let config = Rc::new(RefCell::new(Config::load()));

    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("Recents Filter")
        .default_width(640)
        .default_height(520)
        .build();

    let toolbar = libadwaita::ToolbarView::new();

    let header = gtk4::HeaderBar::new();
    header.set_show_title_buttons(true);
    toolbar.add_top_bar(&header);

    // --- Content ---
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.set_vexpand(true);

    // Folders section.
    content.append(&section_title("Folders to exclude"));

    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");
    list.add_css_class("navigation-sidebar");

    let list_scrolled = gtk4::ScrolledWindow::new();
    list_scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    list_scrolled.set_child(Some(&list));
    list_scrolled.set_vexpand(true);
    content.append(&list_scrolled);

    // Watcher section.
    content.append(&section_title("Watcher"));

    let watcher_box = gtk4::ListBox::new();
    watcher_box.set_selection_mode(gtk4::SelectionMode::None);
    watcher_box.add_css_class("boxed-list");

    // Status row.
    let status_row = libadwaita::ActionRow::new();
    status_row.set_title("Status");
    status_row.set_subtitle("systemd starts a purge whenever recently-used.xbel changes");
    let status_label = gtk4::Label::new(None);
    status_label.set_valign(gtk4::Align::Center);
    status_row.add_suffix(&status_label);
    watcher_box.append(&status_row);

    // Switch row: enable/disable the .path unit.
    let switch_row = libadwaita::SwitchRow::new();
    switch_row.set_title("Watch for changes");
    switch_row.set_subtitle("Automatically purge excluded folders when Recent Files change");
    watcher_box.append(&switch_row);

    // Purge now row.
    let purge_row = libadwaita::ActionRow::new();
    purge_row.set_title("Purge now");
    purge_row.set_subtitle("Remove entries from excluded folders that are already in Recent Files");
    let purge_btn = gtk4::Button::with_label("Purge");
    purge_btn.add_css_class("suggested-action");
    purge_row.add_suffix(&purge_btn);
    watcher_box.append(&purge_row);

    content.append(&watcher_box);

    let clamp = libadwaita::Clamp::new();
    clamp.set_maximum_size(680);
    clamp.set_child(Some(&content));

    let toast_overlay = libadwaita::ToastOverlay::new();
    toast_overlay.set_child(Some(&clamp));

    toolbar.set_content(Some(&toast_overlay));
    window.set_content(Some(&toolbar));

    // Add button: open a folder chooser.
    let add_row = libadwaita::ActionRow::new();
    add_row.set_title("Add folder…");
    add_row.set_activatable(true);
    let add_btn = gtk4::Button::from_icon_name("list-add-symbolic");
    add_btn.add_css_class("flat");
    add_btn.set_valign(gtk4::Align::Center);
    add_row.add_suffix(&add_btn);
    list.append(&add_row);

    refresh_list(&list, &add_row, &config);

    // Initial status + live refresh.
    refresh_status(&status_label, &switch_row);
    glib::timeout_add_local(Duration::from_secs(2), glib::clone!(
        #[strong]
        status_label,
        #[strong]
        switch_row,
        move || {
            refresh_status(&status_label, &switch_row);
            glib::ControlFlow::Continue
        }
    ));

    switch_row.connect_active_notify(glib::clone!(
        #[strong]
        status_label,
        #[strong]
        switch_row,
        move |sw| {
            set_watcher(sw.is_active());
            refresh_status(&status_label, &switch_row);
        }
    ));

    purge_btn.connect_clicked(glib::clone!(
        #[strong]
        toast_overlay,
        move |_| {
            purge_now();
            let toast = libadwaita::Toast::new("Purge finished");
            toast_overlay.add_toast(toast);
        }
    ));
    add_btn.connect_clicked(glib::clone!(
        #[strong]
        list,
        #[strong]
        add_row,
        #[strong]
        config,
        #[strong]
        window,
        move |_| {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Select a folder to keep out of Recent Files");
            dialog.set_initial_folder(Some(&gio::File::for_path(
                std::env::var_os("HOME").unwrap_or_default(),
            )));
            dialog.select_folder(
                Some(&window),
                None::<&gio::Cancellable>,
                glib::clone!(
                    #[strong]
                    list,
                    #[strong]
                    add_row,
                    #[strong]
                    config,
                    move |result| {
                        if let Ok(file) = result {
                            if let Some(path) = file.path() {
                                let mut c = config.borrow_mut();
                                if !c.excluded.contains(&path) {
                                    c.excluded.push(path.clone());
                                    if let Err(e) = c.save() {
                                        log::warn!("cannot save config: {e}");
                                    }
                                }
                                drop(c);
                                refresh_list(&list, &add_row, &config);
                            }
                        }
                    }
                ),
            );
        }
    ));

    window.present();
}

fn refresh_list(
    list: &gtk4::ListBox,
    add_row: &libadwaita::ActionRow,
    config: &Rc<RefCell<Config>>,
) {
    // Remove all rows except the trailing "Add folder…" row.
    while let Some(row) = list.first_child() {
        if row == *add_row.upcast_ref::<gtk4::Widget>() {
            break;
        }
        list.remove(&row);
    }

    let excluded = config.borrow().excluded.clone();
    for path in excluded {
        let row = libadwaita::ActionRow::new();
        row.set_title(path.to_string_lossy().as_ref());
        row.set_subtitle("Excluded from Recent Files");
        row.set_activatable(false);

        let remove_btn = gtk4::Button::from_icon_name("user-trash-symbolic");
        remove_btn.add_css_class("flat");
        remove_btn.set_valign(gtk4::Align::Center);
        row.add_suffix(&remove_btn);

        let path_for_btn = path.clone();
        remove_btn.connect_clicked(glib::clone!(
            #[strong]
            list,
            #[strong]
            add_row,
            #[strong]
            config,
            move |_| {
                config.borrow_mut().excluded.retain(|p| p != &path_for_btn);
                if let Err(e) = config.borrow().save() {
                    log::warn!("cannot save config: {e}");
                }
                refresh_list(&list, &add_row, &config);
            }
        ));

        list.append(&row);
    }

    if config.borrow().excluded.is_empty() {
        let row = libadwaita::ActionRow::new();
        row.set_title("No folders excluded yet");
        row.set_subtitle("Click \"Add folder…\" below.");
        row.set_activatable(false);
        list.append(&row);
    }
}

fn watcher_enabled() -> bool {
    std::process::Command::new("systemctl")
        .args(["--user", "is-enabled", "recents-filterd.path"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "enabled")
        .unwrap_or(false)
}

fn set_watcher(enabled: bool) {
    let action = if enabled { "enable" } else { "disable" };
    let _ = std::process::Command::new("systemctl")
        .args(["--user", action, "--now", "recents-filterd.path"])
        .status();
}

fn purge_now() {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "start", "recents-filterd.service"])
        .status();
}

fn refresh_status(label: &gtk4::Label, switch_row: &libadwaita::SwitchRow) {
    let enabled = watcher_enabled();
    label.set_text(if enabled { "● enabled" } else { "○ disabled" });
    label.set_css_classes(if enabled { &["success"] } else { &["error"] });
    if switch_row.is_active() != enabled {
        switch_row.set_active(enabled);
    }
}
