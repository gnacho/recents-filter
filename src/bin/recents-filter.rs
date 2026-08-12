use gtk4::glib;
use libadwaita::prelude::*;
use recents_filter::ui;

const APP_ID: &str = "org.gnacho.RecentsFilter";

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let app = libadwaita::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(|app| {
        let window = ui::build_window(app);
        window.present();
    });
    app.run()
}
