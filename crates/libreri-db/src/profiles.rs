//! Profiles, PIN attempts and smart collections.

use crate::{Database, Result};
use libreri_core::{Profile, ProfileId, ProfileKind};
use rusqlite::{params, OptionalExtension, Row};

const COLUMNS: &str = "id, name, colour, kind, pin_hash, recovery_hash, allowed_folders, \
     prefs, created_at, last_used";

fn row_to_profile(r: &Row<'_>) -> rusqlite::Result<Profile> {
    let id: String = r.get(0)?;
    let kind: String = r.get(3)?;
    let folders: String = r.get(6)?;
    Ok(Profile {
        id: id.parse::<ProfileId>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        name: r.get(1)?,
        colour: r.get(2)?,
        kind: ProfileKind::parse(&kind).unwrap_or_default(),
        pin_hash: r.get(4)?,
        recovery_hash: r.get(5)?,
        allowed_folders: serde_json::from_str(&folders).unwrap_or_default(),
        prefs: r.get(7)?,
        created_at: r.get(8)?,
        last_used: r.get(9)?,
    })
}

/// A saved search shown in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionRecord {
    pub id: String,
    pub name: String,
    /// JSON `BookQuery`.
    pub query: String,
    pub position: i64,
}

impl Database {
    /// All profiles: the owner first, then by name.
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM profiles
             ORDER BY kind = 'owner' DESC, kind = 'guest', name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map([], row_to_profile)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn profile(&self, id: &ProfileId) -> Result<Option<Profile>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM profiles WHERE id = ?1"),
                [id.to_string()],
                row_to_profile,
            )
            .optional()?)
    }

    /// Adds or replaces a profile (everything except PIN attempts).
    pub fn save_profile(&self, p: &Profile) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profiles(id, name, colour, pin_hash, is_owner, created_at, kind,
                 recovery_hash, allowed_folders, prefs, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, colour=excluded.colour,
                 pin_hash=excluded.pin_hash, is_owner=excluded.is_owner, kind=excluded.kind,
                 recovery_hash=excluded.recovery_hash,
                 allowed_folders=excluded.allowed_folders, prefs=excluded.prefs,
                 last_used=excluded.last_used",
            params![
                p.id.to_string(),
                p.name,
                p.colour,
                p.pin_hash,
                p.kind == ProfileKind::Owner,
                p.created_at,
                p.kind.as_str(),
                p.recovery_hash,
                serde_json::to_string(&p.allowed_folders).unwrap_or_else(|_| "[]".into()),
                p.prefs,
                p.last_used,
            ],
        )?;
        Ok(())
    }

    /// Removes a profile and, through foreign keys, all its personal data.
    pub fn delete_profile(&self, id: &ProfileId) -> Result<()> {
        let id = id.to_string();
        self.conn
            .execute("DELETE FROM profiles WHERE id = ?1", [&id])?;
        self.conn.execute(
            "DELETE FROM library_meta WHERE key = ?1",
            [format!("session:{id}")],
        )?;
        Ok(())
    }

    /// Forgets a profile's reading data but keeps the profile (guests).
    pub fn clear_personal_data(&self, id: &ProfileId) -> Result<()> {
        let id = id.to_string();
        for table in ["book_user", "annotations", "notebooks", "collections"] {
            self.conn
                .execute(&format!("DELETE FROM {table} WHERE profile_id = ?1"), [&id])?;
        }
        self.conn.execute(
            "DELETE FROM library_meta WHERE key = ?1",
            [format!("session:{id}")],
        )?;
        Ok(())
    }

    /// Wrong PINs in a row, and the unix time before which no attempt is
    /// accepted.
    pub fn pin_attempts(&self, id: &ProfileId) -> Result<(u32, i64)> {
        Ok(self
            .conn
            .query_row(
                "SELECT failed_pins, locked_until FROM profiles WHERE id = ?1",
                [id.to_string()],
                |r| Ok((r.get::<_, i64>(0)?.max(0) as u32, r.get(1)?)),
            )
            .optional()?
            .unwrap_or((0, 0)))
    }

    pub fn set_pin_attempts(&self, id: &ProfileId, failed: u32, locked_until: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE profiles SET failed_pins = ?2, locked_until = ?3 WHERE id = ?1",
            params![id.to_string(), i64::from(failed), locked_until],
        )?;
        Ok(())
    }

    /// Adds a profile with a known id if it is missing (used when rebuilding
    /// the database without profile backups).
    pub fn insert_profile(&self, id: &ProfileId, name: &str, now: &str) -> Result<()> {
        let kind = if self.profiles()?.is_empty() {
            ProfileKind::Owner
        } else {
            ProfileKind::Standard
        };
        self.conn.execute(
            "INSERT OR IGNORE INTO profiles(id, name, is_owner, kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id.to_string(),
                name,
                kind == ProfileKind::Owner,
                kind.as_str(),
                now
            ],
        )?;
        Ok(())
    }

    /// Returns the owner, creating one called `name` in a library without
    /// profiles.
    pub fn ensure_owner_profile(&self, name: &str, now: &str) -> Result<ProfileId> {
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM profiles ORDER BY kind = 'owner' DESC, created_at LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing.and_then(|s| s.parse().ok()) {
            return Ok(id);
        }
        let p = Profile::new(name, "graphite", ProfileKind::Owner, now);
        self.save_profile(&p)?;
        Ok(p.id)
    }

    // ---- Smart collections ------------------------------------------------

    pub fn collections(&self, profile: &ProfileId) -> Result<Vec<CollectionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, query, position FROM collections WHERE profile_id = ?1
             ORDER BY position, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([profile.to_string()], |r| {
            Ok(CollectionRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                query: r.get(2)?,
                position: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn save_collection(
        &self,
        profile: &ProfileId,
        c: &CollectionRecord,
        now: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO collections(id, profile_id, name, query, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, query=excluded.query,
                 position=excluded.position
             WHERE collections.profile_id = excluded.profile_id",
            params![c.id, profile.to_string(), c.name, c.query, c.position, now],
        )?;
        Ok(())
    }

    pub fn delete_collection(&self, profile: &ProfileId, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM collections WHERE id = ?1 AND profile_id = ?2",
            params![id, profile.to_string()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-27T10:00:00Z";

    #[test]
    fn profiles_round_trip_and_order() {
        let db = Database::open_in_memory().unwrap();
        let owner = db.ensure_owner_profile("John Smith", NOW).unwrap();
        assert_eq!(db.ensure_owner_profile("Other", NOW).unwrap(), owner);

        let mut jane = Profile::new("Jane Smith", "teal", ProfileKind::Kids, NOW);
        jane.allowed_folders = vec!["Children".into()];
        jane.pin_hash = Some("hash".into());
        db.save_profile(&jane).unwrap();
        let guest = Profile::new("Guest", "graphite", ProfileKind::Guest, NOW);
        db.save_profile(&guest).unwrap();

        let all = db.profiles().unwrap();
        let names: Vec<_> = all.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["John Smith", "Jane Smith", "Guest"]);
        let back = db.profile(&jane.id).unwrap().unwrap();
        assert_eq!(back, jane);
        assert_eq!(back.kind, ProfileKind::Kids);

        db.set_pin_attempts(&jane.id, 5, 99).unwrap();
        assert_eq!(db.pin_attempts(&jane.id).unwrap(), (5, 99));
        db.save_profile(&back).unwrap();
        assert_eq!(db.pin_attempts(&jane.id).unwrap(), (5, 99), "kept on save");

        db.delete_profile(&jane.id).unwrap();
        assert!(db.profile(&jane.id).unwrap().is_none());
    }

    #[test]
    fn collections_belong_to_one_profile() {
        let db = Database::open_in_memory().unwrap();
        let a = db.ensure_owner_profile("A", NOW).unwrap();
        let b = Profile::new("B", "red", ProfileKind::Standard, NOW);
        db.save_profile(&b).unwrap();
        let c = CollectionRecord {
            id: "c1".into(),
            name: "Unread physics".into(),
            query: r#"{"search":"physics"}"#.into(),
            position: 0,
        };
        db.save_collection(&a, &c, NOW).unwrap();
        assert_eq!(db.collections(&a).unwrap(), vec![c.clone()]);
        assert!(db.collections(&b.id).unwrap().is_empty());
        // Another profile cannot overwrite or delete it.
        let mut stolen = c.clone();
        stolen.name = "Mine".into();
        db.save_collection(&b.id, &stolen, NOW).unwrap();
        db.delete_collection(&b.id, "c1").unwrap();
        assert_eq!(db.collections(&a).unwrap()[0].name, "Unread physics");
        db.clear_personal_data(&a).unwrap();
        assert!(db.collections(&a).unwrap().is_empty());
    }
}
