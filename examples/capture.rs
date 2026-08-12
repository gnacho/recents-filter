//! Offscreen screenshot of the main window for README/docs.
//!
//! Renders the real libadwaita UI with a demo config (a random folder) and
//! writes the window to a PNG via a GTK snapshot + cairo — no compositor
//! screenshot needed.
//!
//! Usage (output path via env var: a positional arg would be treated by GTK as
//! a file to open):
//!
//!     CAPTURE_OUT=recents-filter.png cargo run --example capture

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use recents_filter::ui;
use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "org.gnacho.RecentsFilter.Capture";

fn main() -> glib::ExitCode {
    let out = std::env::var("CAPTURE_OUT")
        .unwrap_or_else(|_| "recents-filter.png".to_string());
    let out = Rc::new(RefCell::new(out));

    let app = libadwaita::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(move |app| {
        let window = ui::build_window_capture(app);

        let out = out.clone();
        // The content widget (ToastOverlay) fills the window body.
        let child = window
            .content()
            .expect("window has content")
            .upcast::<gtk4::Widget>();

        // Let the theme and layout settle, then snapshot. Retry until the
        // widget is realized, has a real parent and a non-zero size.
        let attempt = Rc::new(RefCell::new(0u32));
        glib::timeout_add_local(Duration::from_millis(300), glib::clone!(
            #[strong]
            out,
            #[strong]
            window,
            #[strong]
            child,
            #[strong]
            attempt,
            move || {
                let n = *attempt.borrow();
                *attempt.borrow_mut() += 1;

                let Some(parent) = child.parent() else {
                    return glib::ControlFlow::Continue;
                };
                if child.width() == 0 || child.height() == 0 {
                    return glib::ControlFlow::Continue;
                }

                let snapshot = gtk4::Snapshot::new();
                unsafe {
                    gtk4::ffi::gtk_widget_snapshot_child(
                        parent.as_ptr(),
                        child.as_ptr(),
                        snapshot.as_ptr(),
                    );
                }
                if let Some(node) = snapshot.to_node() {
                    let rect = node.bounds();
                    let width = rect.width().ceil() as i32;
                    let height = rect.height().ceil() as i32;
                    if width > 0 && height > 0 {
                        let surface =
                            cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
                                .expect("cairo surface");
                        let ctx = cairo::Context::new(&surface).expect("cairo context");
                        node.draw(&ctx);
                        let path = out.borrow();
                        let mut f = File::create(path.as_str()).expect("create png file");
                        cairo::Surface::write_to_png(&*surface, &mut f).expect("write png");
                        eprintln!("capture written to {} ({}x{})", path, width, height);
                        window.close();
                        return glib::ControlFlow::Break;
                    }
                }
                if n > 40 {
                    eprintln!("giving up: widget never produced a render node");
                    window.close();
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            }
        ));

        window.present();
    });

    app.run()
}
