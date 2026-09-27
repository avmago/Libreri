//! Organising many books at once: bulk edit, and renaming, merging and
//! removing tags and categories. Every change goes through the normal
//! metadata update, so search and the JSON sidecars stay in step.

use crate::{now, sidecar, Error, Library, Result};
use libreri_core::organize::{is_below, rename_category_in, rename_in, similar_tags};
use libreri_core::{Book, BookId, BookQuery, BulkEdit};

impl Library {
    fn tidy_name(name: &str, what: &str) -> Result<String> {
        let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if n.is_empty() {
            return Err(Error::InvalidInput(format!("a {what} needs a name")));
        }
        Ok(n)
    }

    /// Every book, whoever is signed in (library-wide changes).
    fn all_records(&self, query: BookQuery) -> Result<Vec<Book>> {
        let profile = self.viewer();
        self.with_db(|db| db.query_books(&query, &profile))
    }

    /// Saves changed metadata for one book (validated, sidecar written).
    fn store_metadata(&self, book: &Book, metadata: libreri_core::BookMetadata) -> Result<()> {
        let metadata = metadata
            .normalized()
            .map_err(|e| Error::InvalidInput(format!("{}: {e}", book.metadata.title)))?;
        if metadata == book.metadata {
            return Ok(());
        }
        self.with_db(|db| db.update_metadata(&book.id, &metadata, &now()))?;
        sidecar::write(self.layout(), &self.record(&book.id)?)?;
        Ok(())
    }

    /// Applies one edit to many books, in the order given. Returns how many
    /// changed. Nothing is saved if any book would end up invalid.
    pub fn bulk_edit(&self, ids: &[BookId], edit: &BulkEdit) -> Result<usize> {
        self.require_edit()?;
        let mut planned = Vec::with_capacity(ids.len());
        for (i, id) in ids.iter().enumerate() {
            let book = self.book(id)?;
            let mut m = book.metadata.clone();
            edit.apply(&mut m, i).map_err(Error::InvalidInput)?;
            let m = m
                .normalized()
                .map_err(|e| Error::InvalidInput(format!("{}: {e}", book.metadata.title)))?;
            planned.push((book, m));
        }
        let mut changed = 0;
        for (book, m) in planned {
            if m != book.metadata {
                self.store_metadata(&book, m)?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Renames a tag on every book. If `to` already exists the two merge.
    pub fn rename_tag(&self, from: &str, to: &str) -> Result<usize> {
        self.require_edit()?;
        let to = Self::tidy_name(to, "tag")?;
        let books = self.all_records(BookQuery {
            tags: vec![from.to_owned()],
            ..Default::default()
        })?;
        let mut n = 0;
        for b in &books {
            let mut m = b.metadata.clone();
            if rename_in(&mut m.tags, from, &to) {
                self.store_metadata(b, m)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Merges several tags into one.
    pub fn merge_tags(&self, sources: &[String], into: &str) -> Result<usize> {
        let mut n = 0;
        for s in sources {
            if s != into {
                n += self.rename_tag(s, into)?;
            }
        }
        Ok(n)
    }

    /// Removes a tag from every book.
    pub fn delete_tag(&self, tag: &str) -> Result<usize> {
        self.require_edit()?;
        let books = self.all_records(BookQuery {
            tags: vec![tag.to_owned()],
            ..Default::default()
        })?;
        for b in &books {
            let mut m = b.metadata.clone();
            m.tags.retain(|t| !t.eq_ignore_ascii_case(tag));
            self.store_metadata(b, m)?;
        }
        Ok(books.len())
    }

    /// Renames or moves a category and everything below it
    /// ("Science/Physics" → "Physics" also moves "Science/Physics/Optics").
    pub fn rename_category(&self, from: &str, to: &str) -> Result<usize> {
        self.require_edit()?;
        let to = to
            .split('/')
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if to.is_empty() {
            return Err(Error::InvalidInput("a category needs a name".into()));
        }
        if is_below(&to, from) {
            return Err(Error::InvalidInput(
                "a category cannot move inside itself".into(),
            ));
        }
        let books = self.all_records(BookQuery {
            category: Some(from.to_owned()),
            ..Default::default()
        })?;
        let mut n = 0;
        for b in &books {
            let mut m = b.metadata.clone();
            if rename_category_in(&mut m.categories, from, &to) {
                self.store_metadata(b, m)?;
                n += 1;
            }
        }
        Ok(n)
    }

    /// Removes a category (and those below it) from every book.
    pub fn delete_category(&self, path: &str) -> Result<usize> {
        self.require_edit()?;
        let books = self.all_records(BookQuery {
            category: Some(path.to_owned()),
            ..Default::default()
        })?;
        for b in &books {
            let mut m = b.metadata.clone();
            m.categories
                .retain(|c| !c.eq_ignore_ascii_case(path) && !is_below(c, path));
            self.store_metadata(b, m)?;
        }
        Ok(books.len())
    }

    /// Groups of tags that look like the same thing, for merging.
    pub fn similar_tags(&self) -> Result<Vec<Vec<(String, u32)>>> {
        Ok(similar_tags(&self.facets()?.tags))
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::{BookQuery, BulkEdit};

    fn setup() -> (tempfile::TempDir, Library) {
        let (d, lib) = library();
        let books = lib.layout().books_dir();
        for (i, name) in ["a", "b", "c"].iter().enumerate() {
            let p = books.join(format!("{name}.md"));
            std::fs::write(
                &p,
                format!(
                    "---\ntitle: Book {name}\ntags: [ML, Physics{}]\ncategories: [Science/Physics]\n---\nx\n",
                    if i == 0 { ", machine-learning" } else { "" }
                ),
            )
            .unwrap();
        }
        lib.scan(&NoProgress).unwrap();
        (d, lib)
    }

    fn ids(lib: &Library) -> Vec<libreri_core::BookId> {
        lib.books(&BookQuery::default())
            .unwrap()
            .into_iter()
            .map(|b| b.id)
            .collect()
    }

    #[test]
    fn bulk_edit_changes_every_selected_book() {
        let (_d, lib) = setup();
        let ids = ids(&lib);
        let edit = BulkEdit {
            publisher: Some("Acme".into()),
            series: Some("Lectures".into()),
            number_series: true,
            add_tags: vec!["Course".into()],
            ..Default::default()
        };
        assert_eq!(lib.bulk_edit(&ids, &edit).unwrap(), 3);
        let books = lib.books(&BookQuery::default()).unwrap();
        assert!(books
            .iter()
            .all(|b| b.metadata.publisher.as_deref() == Some("Acme")));
        let numbers: Vec<_> = ids
            .iter()
            .map(|id| lib.book(id).unwrap().metadata.series_number)
            .collect();
        assert_eq!(numbers, [Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(
            lib.books(&BookQuery {
                search: Some("course".into()),
                ..Default::default()
            })
            .unwrap()
            .len(),
            3,
            "search sees the new tag"
        );
        // A bad value changes nothing at all.
        let bad = BulkEdit {
            year: Some("soon".into()),
            publisher: Some("Other".into()),
            ..Default::default()
        };
        assert!(lib.bulk_edit(&ids, &bad).is_err());
        assert_eq!(
            lib.book(&ids[0]).unwrap().metadata.publisher.as_deref(),
            Some("Acme")
        );
        // Sidecars follow, so a rebuild keeps the edit.
        lib.rebuild_index(&NoProgress).unwrap();
        assert_eq!(
            lib.book(&ids[1]).unwrap().metadata.publisher.as_deref(),
            Some("Acme")
        );
    }

    #[test]
    fn tags_rename_merge_and_delete() {
        let (_d, lib) = setup();
        let groups = lib.similar_tags().unwrap();
        assert!(groups.is_empty(), "{groups:?}");
        assert_eq!(
            lib.merge_tags(&["machine-learning".into()], "ML").unwrap(),
            1
        );
        assert_eq!(lib.rename_tag("ml", "Machine learning").unwrap(), 3);
        let tags: Vec<_> = lib.facets().unwrap().tags;
        assert!(tags.contains(&("Machine learning".into(), 3)), "{tags:?}");
        assert!(!tags.iter().any(|(t, _)| t == "machine-learning"));
        assert_eq!(lib.delete_tag("physics").unwrap(), 3);
        assert_eq!(
            lib.facets().unwrap().tags,
            vec![("Machine learning".into(), 3)]
        );
    }

    #[test]
    fn categories_move_and_delete() {
        let (_d, lib) = setup();
        assert_eq!(
            lib.rename_category("Science", "Natural sciences").unwrap(),
            3
        );
        assert_eq!(
            lib.facets().unwrap().categories,
            vec![("Natural sciences/Physics".into(), 3)]
        );
        assert!(lib
            .rename_category("Natural sciences", "Natural sciences/Inside")
            .is_err());
        assert_eq!(lib.delete_category("natural sciences").unwrap(), 3);
        assert!(lib.facets().unwrap().categories.is_empty());
    }
}
