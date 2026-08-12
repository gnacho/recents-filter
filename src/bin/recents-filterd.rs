use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use recents_filter::config::Config;
use recents_filter::purge;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(300);

fn purge_with_config() {
    let config = Config::load();
    match purge::purge(&config.excluded) {
        Ok(n) => {
            if n > 0 {
                log::info!("purged {n} recent entries");
            }
        }
        Err(e) => log::warn!("purge failed: {e}"),
    }
}

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let xbel: PathBuf = purge::recent_file();
    let config_dir = Config::config_dir();

    // Initial purge on start, so entries are clean even if the daemon was off.
    purge_with_config();

    let (tx, rx) = mpsc::channel();
    let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
        Ok(w) => w,
        Err(e) => {
            log::error!("cannot create inotify watcher: {e}");
            return glib::ExitCode::FAILURE;
        }
    };

    // Watch the data dir where recently-used.xbel lives.
    if let Some(parent) = xbel.parent() {
        if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
            log::error!("cannot watch {}: {e}", parent.display());
            return glib::ExitCode::FAILURE;
        }
    }
    // Watch the config dir so edits from the GUI are picked up live.
    if config_dir.exists() {
        if let Err(e) = watcher.watch(&config_dir, RecursiveMode::NonRecursive) {
            log::warn!("cannot watch config dir {}: {e}", config_dir.display());
        }
    }

    log::info!(
        "watching {} and {}",
        xbel.display(),
        Config::config_path().display()
    );

    for event in rx.into_iter().flatten() {
        use notify::EventKind::*;
        let relevant = match event.kind {
            Create(_) | Modify(_) | Remove(_) | Access(_) => true,
            _ => false,
        };
        if !relevant {
            continue;
        }

        let touched = event
            .paths
            .iter()
            .any(|p| p == &xbel || p == &Config::config_path());
        if !touched {
            continue;
        }

        // Debounce: wait for writes to settle, then purge.
        std::thread::sleep(DEBOUNCE);
        purge_with_config();
    }

    glib::ExitCode::SUCCESS
}
