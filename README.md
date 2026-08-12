# Recents Filter

Keep selected folders out of the GNOME Recent Files list, in every app.

![GNU Affero General Public License v3](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)

## Why this exists

GNOME has no per-folder exclusion for Recent Files. GTK apps record everything
you open in `~/.local/share/recently-used.xbel`, and there is no setting that
says "this folder never shows up here". The usual workaround — hiding a folder
by prefixing its name with a dot — only hides its entries from Nautilus and the
GTK file picker views. The entries still land in the xbel, third-party apps
show them, and toggling *Show hidden files* exposes everything again.

Recents Filter closes that gap with two small pieces:

- **A GUI (GTK4/libadwaita)** to manage the list of excluded folders. It lives
  in `~/.config/recents-filter/config.json`.
- **A background daemon (inotify)** that watches `recently-used.xbel`. The
  moment any app rewrites it, the daemon strips every bookmark whose path is
  under an excluded folder and writes the file back atomically. Runs as a
  systemd user service, so it survives closing the GUI and works across
  session restarts.

## Install

```bash
cargo build --release
cp target/release/recents-filter target/release/recents-filterd ~/.local/bin/
mkdir -p ~/.config/systemd/user ~/.local/share/applications ~/.local/share/metainfo
cp data/recents-filterd.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now recents-filterd
cp data/org.gnacho.RecentsFilter.desktop ~/.local/share/applications/
cp data/org.gnacho.RecentsFilter.metainfo.xml ~/.local/share/metainfo/
update-desktop-database ~/.local/share/applications
```

Then launch **Recents Filter** from your app grid and add the folders you want
to keep out of Recent Files.

## How it behaves

- **Instant**: the daemon reacts to the xbel with a ~300 ms debounce, so a file
  opened in an excluded folder disappears from Recent Files within a second.
- **Atomic**: the xbel is rewritten through a temp file + rename, so a GTK app
  reading it concurrently never sees a partial file.
- **Private**: the rewritten xbel keeps the `0600` mode GTK uses for it.
- **Converges**: when a GTK app adds a non-excluded recent, it rewrites the
  whole xbel from its in-memory list — including excluded entries. The daemon
  purges them again. It settles in seconds, with no loop.

## Development

```bash
cargo test    # unit tests for the xbel parser/purger
cargo build   # debug build of both binaries
```

## License

[AGPL-3.0](LICENSE)
