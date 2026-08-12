use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Returns the standard location of the GTK recent files store.
pub fn recent_file() -> PathBuf {
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| glib::user_data_dir());
    dir.join("recently-used.xbel")
}

fn path_is_excluded(path: &Path, excluded: &[PathBuf]) -> bool {
    excluded.iter().any(|ex| path.starts_with(ex))
}

/// Percent-decodes a `file://` URI into a local path.
fn uri_to_path(href: &str) -> Option<PathBuf> {
    let url = url::Url::parse(href).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

/// Parses `recently-used.xbel` and returns the ranges of `<bookmark>` elements
/// whose `href` points into any excluded folder.
fn bookmark_ranges_to_purge(xbel: &str, excluded: &[PathBuf]) -> Vec<std::ops::Range<usize>> {
    let doc = match roxmltree::Document::parse(xbel) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut ranges = Vec::new();
    for node in doc.descendants() {
        if !node.is_element() || node.tag_name().name() != "bookmark" {
            continue;
        }
        let Some(href) = node.attribute("href") else { continue };
        let Some(path) = uri_to_path(href) else { continue };
        if path_is_excluded(&path, excluded) {
            ranges.push(node.range());
        }
    }
    ranges
}

/// Removes from the xbel every bookmark under an excluded folder.
///
/// Returns the number of entries removed. Writes atomically (temp file +
/// rename) so a concurrent GTK reader never sees a partial file. If nothing
/// needs removing the file is left untouched.
pub fn purge(excluded: &[PathBuf]) -> Result<usize, String> {
    let path = recent_file();
    let contents = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    let ranges = bookmark_ranges_to_purge(&contents, excluded);
    let removed = ranges.len();
    if ranges.is_empty() {
        return Ok(0);
    }

    // Apply removals from the end so earlier byte offsets stay valid.
    let mut result = contents;
    for range in ranges.into_iter().rev() {
        result.replace_range(range, "");
    }

    // Atomic write: temp file in the same dir, fsync, then rename. Keep the
    // private 0600 mode that GTK uses for this file.
    let tmp = path.with_extension("xbel.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(result.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_xbel() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0" xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info" xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks">
  <bookmark href="file:///home/user/Descargas/.eso/movie.mkv" added="2026-07-21T14:45:20Z" modified="2026-07-21T14:45:20Z" visited="2026-07-21T14:45:20Z">
    <title>movie.mkv</title>
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="video/x-matroska"/>
      </metadata>
    </info>
  </bookmark>
  <bookmark href="file:///home/user/Documentos/report.pdf" added="2026-07-21T14:45:20Z" modified="2026-07-21T14:45:20Z" visited="2026-07-21T14:45:20Z">
    <title>report.pdf</title>
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="application/pdf"/>
      </metadata>
    </info>
  </bookmark>
</xbel>"#
        .to_string()
    }

    #[test]
    fn uri_to_path_decodes_percent_encoding() {
        let p = uri_to_path("file:///home/user/Descargas/.eso/movie%20with%20spaces.mkv").unwrap();
        assert_eq!(p, PathBuf::from("/home/user/Descargas/.eso/movie with spaces.mkv"));
    }

    #[test]
    fn uri_to_path_ignores_non_file_schemes() {
        assert!(uri_to_path("https://example.com/x").is_none());
        assert!(uri_to_path("not a uri").is_none());
    }

    #[test]
    fn bookmark_ranges_only_matches_excluded_folder() {
        let xbel = sample_xbel();
        let excluded = vec![PathBuf::from("/home/user/Descargas/.eso")];
        let ranges = bookmark_ranges_to_purge(&xbel, &excluded);
        assert_eq!(ranges.len(), 1);
        // The removed range covers the full <bookmark>...</bookmark> element.
        let removed = &xbel[ranges[0].clone()];
        assert!(removed.starts_with("<bookmark href=\"file:///home/user/Descargas/.eso/movie.mkv\""));
        assert!(removed.ends_with("</bookmark>"));
    }

    #[test]
    fn purge_writes_clean_file() {
        let xbel = sample_xbel();
        let excluded = vec![PathBuf::from("/home/user/Descargas/.eso")];
        let mut result = xbel;
        let ranges = bookmark_ranges_to_purge(&result, &excluded);
        for range in ranges.into_iter().rev() {
            result.replace_range(range, "");
        }
        assert!(!result.contains("movie.mkv"));
        assert!(result.contains("report.pdf"));
        assert!(result.contains("</xbel>"));
    }

    #[test]
    fn purge_integration_writes_0600_and_is_valid_xml() {
        let dir = std::env::temp_dir().join(format!("recents-filter-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let xbel_path = dir.join("recently-used.xbel");
        std::fs::write(&xbel_path, sample_xbel()).unwrap();

        // Point recent_file() at the temp file via XDG_DATA_HOME.
        std::env::set_var("XDG_DATA_HOME", &dir);

        let excluded = vec![PathBuf::from("/home/user/Descargas/.eso")];
        let removed = purge(&excluded).unwrap();
        assert_eq!(removed, 1);

        let cleaned = std::fs::read_to_string(&xbel_path).unwrap();
        assert!(!cleaned.contains("movie.mkv"));
        assert!(cleaned.contains("report.pdf"));

        // Still parseable XML after the byte surgery.
        roxmltree::Document::parse(&cleaned).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&xbel_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
