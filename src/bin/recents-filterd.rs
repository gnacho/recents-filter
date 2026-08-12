use recents_filter::config::Config;
use recents_filter::purge;

/// One-shot: purge excluded folders from recently-used.xbel and exit.
///
/// The watching is done by systemd: a recents-filterd.path unit starts this
/// service whenever the xbel changes. There is no resident process.
fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Config::load();
    match purge::purge(&config.excluded) {
        Ok(0) => {}
        Ok(n) => log::info!("purged {n} recent entries"),
        Err(e) => {
            log::warn!("purge failed: {e}");
            return glib::ExitCode::FAILURE;
        }
    }

    glib::ExitCode::SUCCESS
}
