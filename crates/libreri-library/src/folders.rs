//! Real folders under `Books/`. What you see in the sidebar is exactly what
//! is on disk; creating, renaming and moving folders changes the disk.

use crate::paths::{folder_abs, is_hidden, validate_name};
use crate::{sidecar, Error, Library, Result};
use libreri_core::book::folder_of;
use libreri_core::BookQuery;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderNode {
    pub name: String,
    /// Relative to `Books/`, `/`-separated ("Science/Physics").
    pub path: String,
    /// Books directly in this folder.
    pub book_count: u32,
    /// Books in this folder and all folders below it.
    pub total_count: u32,
    pub children: Vec<FolderNode>,
}

fn walk(dir: &Path, rel: &str, counts: &HashMap<String, u32>) -> Vec<FolderNode> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nodes: Vec<FolderNode> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !is_hidden(&e.file_name()))
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let children = walk(&e.path(), &path, counts);
            let book_count = counts.get(&path).copied().unwrap_or(0);
            let total_count = book_count + children.iter().map(|c| c.total_count).sum::<u32>();
            FolderNode {
                name,
                path,
                book_count,
                total_count,
                children,
            }
        })
        .collect();
    nodes.sort_by(|a, b| {
        libreri_core::query::sort_title(&a.name).cmp(&libreri_core::query::sort_title(&b.name))
    });
    nodes
}

/// Keeps folders inside `scope`, and their parents so the tree still shows
/// where they are. Parents outside the scope show only allowed books.
fn prune(nodes: Vec<FolderNode>, scope: &[String]) -> Vec<FolderNode> {
    let inside = |p: &str| {
        scope
            .iter()
            .any(|s| p == s || p.starts_with(&format!("{s}/")))
    };
    let above = |p: &str| scope.iter().any(|s| s.starts_with(&format!("{p}/")));
    nodes
        .into_iter()
        .filter_map(|mut n| {
            if inside(&n.path) {
                Some(n)
            } else if above(&n.path) {
                n.children = prune(n.children, scope);
                n.book_count = 0;
                n.total_count = n.children.iter().map(|c| c.total_count).sum();
                Some(n)
            } else {
                None
            }
        })
        .collect()
}

impl Library {
    /// The folder tree under `Books/` with book counts.
    pub fn folders(&self) -> Result<Vec<FolderNode>> {
        let mut counts: HashMap<String, u32> = HashMap::new();
        for rec in self.with_db(|db| db.file_records())? {
            *counts
                .entry(folder_of(&rec.rel_path).to_owned())
                .or_default() += 1;
        }
        let tree = walk(&self.layout().books_dir(), "", &counts);
        let scope = self.scope();
        Ok(if scope.is_empty() {
            tree
        } else {
            prune(tree, &scope)
        })
    }

    /// Creates `name` inside `parent` (relative to `Books/`). Returns the new
    /// folder's path.
    pub fn create_folder(&self, parent: &str, name: &str) -> Result<String> {
        self.require_edit()?;
        let name = validate_name(name)?;
        let parent_abs = folder_abs(self.layout(), parent)?;
        if !parent_abs.is_dir() {
            return Err(Error::InvalidInput(
                "the parent folder no longer exists".into(),
            ));
        }
        let dir = parent_abs.join(&name);
        if dir.exists() {
            return Err(Error::NameTaken(name));
        }
        fs::create_dir(&dir)?;
        Ok(join(parent, &name))
    }

    /// Renames a folder. Returns its new path.
    pub fn rename_folder(&self, path: &str, new_name: &str) -> Result<String> {
        self.require_edit()?;
        let name = validate_name(new_name)?;
        let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        self.relocate_folder(path, &join(parent, &name))
    }

    /// Moves a folder (and everything in it) into `new_parent`.
    pub fn move_folder(&self, path: &str, new_parent: &str) -> Result<String> {
        self.require_edit()?;
        let name = path.rsplit('/').next().unwrap_or(path);
        let target = join(new_parent, name);
        if new_parent == path || new_parent.starts_with(&format!("{path}/")) {
            return Err(Error::InvalidInput(
                "a folder cannot be moved into itself".into(),
            ));
        }
        self.relocate_folder(path, &target)
    }

    fn relocate_folder(&self, from: &str, to: &str) -> Result<String> {
        let from = from.trim_matches('/');
        if from.is_empty() {
            return Err(Error::InvalidInput("choose a folder".into()));
        }
        if from == to {
            return Ok(to.to_owned());
        }
        let from_abs = folder_abs(self.layout(), from)?;
        let to_abs = folder_abs(self.layout(), to)?;
        if !from_abs.is_dir() {
            return Err(Error::InvalidInput("that folder no longer exists".into()));
        }
        // Case-only renames ("physics" → "Physics") are fine on every OS.
        let case_only = from.eq_ignore_ascii_case(to);
        if to_abs.exists() && !case_only {
            return Err(Error::NameTaken(
                to.rsplit('/').next().unwrap_or(to).to_owned(),
            ));
        }
        if case_only {
            let tmp = from_abs.with_file_name(format!(".libreri-rename-{}", std::process::id()));
            fs::rename(&from_abs, &tmp)?;
            fs::rename(&tmp, &to_abs)?;
        } else {
            fs::rename(&from_abs, &to_abs)?;
        }
        self.with_db(|db| db.move_folder_paths(&format!("Books/{from}"), &format!("Books/{to}")))?;
        self.follow_folder_change(from, Some(to))?;
        for book in self.books(&BookQuery {
            folder: Some(to.to_owned()),
            include_subfolders: true,
            ..Default::default()
        })? {
            sidecar::write(self, &book)?;
        }
        Ok(to.to_owned())
    }

    /// Moves a folder and every book in it to the system trash. Returns how
    /// many books went with it.
    pub fn trash_folder(&self, path: &str) -> Result<usize> {
        self.require_edit()?;
        let path = path.trim_matches('/');
        if path.is_empty() {
            return Err(Error::InvalidInput("choose a folder".into()));
        }
        let abs = folder_abs(self.layout(), path)?;
        let books = self.books(&BookQuery {
            folder: Some(path.to_owned()),
            include_subfolders: true,
            ..Default::default()
        })?;
        if abs.exists() {
            trash::delete(&abs).map_err(|e| Error::Trash(e.to_string()))?;
        }
        for b in &books {
            self.with_db(|db| db.delete_book(&b.id))?;
        }
        self.follow_folder_change(path, None)?;
        Ok(books.len())
    }
}

fn join(parent: &str, name: &str) -> String {
    let parent = parent.trim_matches('/');
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;

    #[test]
    fn create_rename_move_and_count() {
        let (dir, lib) = library();
        lib.create_folder("", "Science").unwrap();
        lib.create_folder("Science", "physics").unwrap();
        lib.create_folder("", "Fiction").unwrap();
        assert!(matches!(
            lib.create_folder("", "Science"),
            Err(Error::NameTaken(_))
        ));
        let src = md_book(dir.path(), "q.md", "Quantum");
        lib.import(
            &ImportRequest {
                sources: vec![src],
                folder: "Science/physics".into(),
                mode: ImportMode::Move,
            },
            &NoProgress,
        )
        .unwrap();

        let tree = lib.folders().unwrap();
        assert_eq!(
            tree.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            vec!["Fiction", "Science"]
        );
        assert_eq!(tree[1].total_count, 1);
        assert_eq!(tree[1].children[0].book_count, 1);

        // Case-only rename, then move into another folder.
        assert_eq!(
            lib.rename_folder("Science/physics", "Physics").unwrap(),
            "Science/Physics"
        );
        assert_eq!(
            lib.move_folder("Science/Physics", "Fiction").unwrap(),
            "Fiction/Physics"
        );
        let b = &lib.books(&BookQuery::default()).unwrap()[0];
        assert_eq!(b.rel_path, "Books/Fiction/Physics/q.md");
        assert!(lib.layout().root().join(&b.rel_path).is_file());
        assert!(matches!(
            lib.move_folder("Fiction", "Fiction/Physics"),
            Err(Error::InvalidInput(_))
        ));
        assert!(lib.create_folder("", "a/b").is_err());
    }

    #[test]
    fn kids_folders_follow_renames_and_case_matters() {
        let (_d, lib) = library();
        let books = lib.layout().books_dir();
        md_book(&books, "Kids/a.md", "Gruffalo");
        md_book(&books, "kids2/b.md", "Other");
        lib.scan(&NoProgress).unwrap();
        let kid = lib
            .create_profile("John", "blue", libreri_core::ProfileKind::Kids, None)
            .unwrap();
        lib.set_allowed_folders(&kid.id, vec!["Kids".into(), "Kids/Sub".into()])
            .unwrap();
        lib.rename_folder("Kids", "Children").unwrap();
        let p = lib.load_profile(&kid.id).unwrap();
        assert_eq!(p.allowed_folders, vec!["Children", "Children/Sub"]);
        lib.move_folder("kids2", "Children").unwrap();
        assert_eq!(lib.load_profile(&kid.id).unwrap().allowed_folders.len(), 2);
        let _ = lib.trash_folder("Children");
        if !lib.layout().books_dir().join("Children").exists() {
            assert!(lib
                .load_profile(&kid.id)
                .unwrap()
                .allowed_folders
                .is_empty());
        }
    }
}
