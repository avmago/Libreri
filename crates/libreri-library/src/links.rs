//! Links from a place in a book: the pictures of linked videos
//! and pages (`Notes/<profile>/Links/`) and offline copies of web pages
//! (`Notes/<profile>/Web pages/`), linked by "link" annotations.

use crate::{Error, Library, Result};
use std::fs;

pub const LINK_PICTURE_DIR: &str = "Links";
pub const WEB_COPY_DIR: &str = "Web pages";

fn file_name(title: &str) -> String {
    let t = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let t: String = t.chars().take(80).collect();
    let t = if t.trim().is_empty() {
        chrono::Local::now()
            .format("Link %Y-%m-%d %H-%M")
            .to_string()
    } else {
        t
    };
    crate::reading::notes_folder_name(&t)
}

impl Library {
    fn save_note_file(&self, dir: &str, title: &str, ext: &str, bytes: &[u8]) -> Result<String> {
        let dir = self.notes_folder()?.join(dir);
        fs::create_dir_all(&dir)?;
        let file = crate::paths::unique_path(&dir, &format!("{}.{ext}", file_name(title)));
        crate::paths::write_atomic(&file, bytes)?;
        crate::paths::rel_of(self.layout(), &file).ok_or(Error::BookNotFound)
    }

    /// Saves a link's picture (JPEG, PNG, GIF, WebP or AVIF). Returns its
    /// path in the library.
    pub fn save_link_picture(&self, title: &str, bytes: &[u8], mime: &str) -> Result<String> {
        let ext = match mime {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/avif" => "avif",
            _ => return Err(Error::InvalidInput("not a picture".into())),
        };
        self.save_note_file(LINK_PICTURE_DIR, title, ext, bytes)
    }

    /// Saves an offline copy of a web page. Returns its path in the library.
    pub fn save_web_copy(&self, title: &str, html: &str) -> Result<String> {
        self.save_note_file(WEB_COPY_DIR, title, "html", html.as_bytes())
    }

    /// A file in the signed-in profile's own notes folder.
    pub fn own_note_file(&self, rel: &str) -> Result<std::path::PathBuf> {
        let dir = self.own_notes_dir()?;
        self.layout()
            .resolve_relative(rel)
            .filter(|p| p.starts_with(&dir))
            .ok_or_else(|| Error::NotAllowed("that file belongs to someone else".into()))
    }

    /// The path in the library of a file inside it (for linking videos and
    /// recordings kept in the library, so the link travels with it).
    pub fn relative_path(&self, path: &std::path::Path) -> Option<String> {
        crate::paths::rel_of(self.layout(), path)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::library;

    #[test]
    fn keeps_link_files_in_the_notes_folder() {
        let (_d, lib) = library();
        let pic = lib
            .save_link_picture("Keepers of the light", b"\x89PNGxx", "image/png")
            .unwrap();
        assert!(
            pic.starts_with("Notes/") && pic.ends_with("/Links/Keepers of the light.png"),
            "{pic}"
        );
        let copy = lib
            .save_web_copy("Lighthouses | Sea Wiki", "<p>hi</p>")
            .unwrap();
        assert!(
            copy.contains("/Web pages/") && copy.ends_with(".html"),
            "{copy}"
        );
        let again = lib
            .save_web_copy("Lighthouses | Sea Wiki", "<p>hi</p>")
            .unwrap();
        assert_ne!(copy, again);
        assert!(lib
            .save_link_picture("x", b"<svg", "image/svg+xml")
            .is_err());
    }
}
