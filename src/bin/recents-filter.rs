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

fn build_ui(app: &libadwaita::Application) {
    let config = Rc::new(RefCell::new(Config::load()));

    let window = libadwaita::ApplicationWindow::builder()
        .application(app)
        .title("Recents Filter")
        .default_width(640)
        .default_height(480)
        .build();

    let toolbar = libadwaita::ToolbarView::new();

    let header = gtk4::HeaderBar::new();
    header.set_show_title_buttons(true);
    toolbar.add_top_bar(&header);

    // --- Content: a list of excluded folders + add button ---
    let list = gtk4::ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::None);
    list.add_css_class("boxed-list");

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scrolled.set_child(Some(&list));
    scrolled.set_vexpand(true);

    let clamp = libadwaita::Clamp::new();
    clamp.set_maximum_size(640);
    clamp.set_child(Some(&scrolled));

    toolbar.set_content(Some(&clamp));

    let footer = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    footer.add_css_class("boxed-list");

    let add_row = libadwaita::ActionRow::new();
    add_row.set_title("Add folder to exclude");
    add_row.set_subtitle("Its contents will never appear in Recent Files");
    add_row.set_activatable(true);

    let add_btn = gtk4::Button::with_label("Add…");
    add_btn.add_css_class("suggested-action");
    add_row.add_suffix(&add_btn);
    footer.append(&add_row);

    let status_row = libadwaita::ActionRow::new();
    status_row.set_title("Watcher");
    status_row.set_subtitle("systemd triggers a purge whenever recently-used.xbel changes");
    let status_label = gtk4::Label::new(None);
    let enable_btn = gtk4::Button::with_label("Enable watcher");
    enable_btn.add_css_class("suggested-action");
    enable_btn.set_visible(false);
    let purge_btn = gtk4::Button::with_label("Purge now");
    purge_btn.add_css_class("suggested-action");
    status_row.add_suffix(&purge_btn);
    status_row.add_suffix(&enable_btn);
    status_row.add_suffix(&status_label);
    footer.append(&status_row);

    toolbar.add_bottom_bar(&footer);
    window.set_content(Some(&toolbar));

    refresh_list(&list, &config);

    // Refresh watcher status periodically so the label follows live state.
    refresh_status(&status_label, &enable_btn);
    glib::timeout_add_local(Duration::from_secs(2), glib::clone!(
        #[strong]
        status_label,
        #[strong]
        enable_btn,
        move || {
            refresh_status(&status_label, &enable_btn);
            glib::ControlFlow::Continue
        }
    ));

    enable_btn.connect_clicked(glib::clone!(
        #[strong]
        status_label,
        #[strong]
        enable_btn,
        move |_| {
            enable_watcher();
            refresh_status(&status_label, &enable_btn);
        }
    ));

    purge_btn.connect_clicked(glib::clone!(
        #[strong]
        status_label,
        #[strong]
        enable_btn,
        move |_| {
            purge_now();
            refresh_status(&status_label, &enable_btn);
        }
    ));

    // Add button: open a folder chooser.
    add_btn.connect_clicked(glib::clone!(
        #[strong]
        list,
        #[strong]
        config,
        #[strong]
        window,
        move |_| {
            let dialog = gtk4::FileDialog::new();
            dialog.set_title("Select a folder to keep out of Recent Files");
            dialog.set_initial_folder(Some(&gio::File::for_path(std::env::var_os("HOME").unwrap_or_default())));
            dialog.select_folder(
                Some(&window),
                None::<&gio::Cancellable>,
                glib::clone!(
                    #[strong]
                    list,
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
                                refresh_list(&list, &config);
                            }
                        }
                    }
                ),
            );
        }
    ));

    window.present();
}

fn refresh_list(list: &gtk4::ListBox, config: &Rc<RefCell<Config>>) {
    while let Some(row) = list.first_child() {
        list.remove(&row);
    }

    let excluded = config.borrow().excluded.clone();
    for path in excluded {
        let row = libadwaita::ActionRow::new();
        row.set_title(path.to_string_lossy().as_ref());
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
            config,
            move |_| {
                config.borrow_mut().excluded.retain(|p| p != &path_for_btn);
                if let Err(e) = config.borrow().save() {
                    log::warn!("cannot save config: {e}");
                }
                refresh_list(&list, &config);
            }
        ));

        list.append(&row);
    }

    // Empty state.
    if config.borrow().excluded.is_empty() {
        let row = libadwaita::ActionRow::new();
        row.set_title("No folders excluded yet");
        row.set_subtitle("Click \"Add folder to exclude\" above.");
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

fn enable_watcher() {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "recents-filterd.path"])
        .status();
}

fn purge_now() {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "start", "recents-filterd.service"])
        .status();
}

fn refresh_status(label: &gtk4::Label, enable_btn: &gtk4::Button) {
    let enabled = watcher_enabled();
    label.set_text(if enabled { "● enabled" } else { "○ disabled" });
    label.set_css_classes(if enabled { &["success"] } else { &["warning"] });
    enable_btn.set_visible(!enabled);
    enable_btn.set_sensitive(!enabled);
}
