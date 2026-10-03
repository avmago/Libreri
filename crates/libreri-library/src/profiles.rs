//! Profiles: signing in with an optional 6-digit PIN, locking, managing
//! profiles, and each profile's smart collections.
//!
//! Every profile is also written to `.library-data/profiles/<id>.json`, so a
//! rebuilt database keeps the same people, PINs and preferences.

use crate::paths::write_atomic;
use crate::{now, Error, Library, Result, Session};
use libreri_core::profile::{validate_profile_name, PROFILE_COLOURS};
use libreri_core::{BookQuery, Profile, ProfileId, ProfileKind};
use libreri_db::CollectionRecord;
use libreri_profiles::{
    hash_secret, lockout_seconds, new_recovery_code, normalise_code, validate_pin, verify_secret,
    PinError, FREE_ATTEMPTS,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A saved search in the sidebar.
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub query: BookQuery,
}

/// A collection as backed up in `.library-data/profiles/<id>.collections.json`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionBackup {
    pub id: String,
    pub name: String,
    pub query: String,
    pub position: i64,
}

/// What `set_pin` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinChange {
    /// For the owner: a new recovery code to write down, shown once.
    pub recovery_code: Option<String>,
}

fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn pin_error(e: PinError) -> Error {
    Error::InvalidInput(e.to_string())
}

fn protect(secret: &str) -> Result<String> {
    hash_secret(secret).map_err(pin_error)
}

/// Refuses `name` when another profile (not `except`) has it, or when the
/// two would share one `Notes/` folder.
fn check_name_free(all: &[Profile], except: Option<&ProfileId>, name: &str) -> Result<()> {
    for o in all.iter().filter(|o| Some(&o.id) != except) {
        if o.name.to_lowercase() == name.to_lowercase() {
            return Err(Error::InvalidInput(format!(
                "there is already a profile called “{name}”"
            )));
        }
        if crate::reading::same_notes_folder(&o.name, name) {
            return Err(Error::InvalidInput(format!(
                "“{name}” is too close to “{}”: they would share a notes folder; please choose another name",
                o.name
            )));
        }
    }
    Ok(())
}

fn check_colour(colour: &str) -> Result<()> {
    if PROFILE_COLOURS.contains(&colour) {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!("unknown colour “{colour}”")))
    }
}

impl Library {
    pub(crate) fn profiles_dir(&self) -> PathBuf {
        self.layout().data_dir().join("profiles")
    }

    pub(crate) fn profile_backup(&self, p: &Profile) -> Result<()> {
        let dir = self.profiles_dir();
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_vec_pretty(p).map_err(std::io::Error::other)?;
        write_atomic(&dir.join(format!("{}.json", p.id)), &json)?;
        Ok(())
    }

    /// Writes every profile's backup (after opening, so older libraries
    /// get them too).
    pub(crate) fn backup_all_profiles(&self) -> Result<()> {
        for p in self.with_db(|db| db.profiles())? {
            self.profile_backup(&p)?;
        }
        Ok(())
    }

    /// Puts profiles from their backups into a freshly rebuilt database.
    pub(crate) fn restore_profiles(&self, db: &libreri_db::Database) -> Result<usize> {
        let Ok(entries) = fs::read_dir(self.profiles_dir()) else {
            return Ok(0);
        };
        let mut n = 0;
        let mut collections = Vec::new();
        for e in entries.flatten() {
            let Ok(text) = fs::read_to_string(e.path()) else {
                continue;
            };
            let name = e.file_name().to_string_lossy().into_owned();
            if name.ends_with(crate::study::SUFFIX) || name.ends_with(crate::study::REVIEW_SUFFIX) {
                // The reading calendar: read where it lies, nothing to restore.
                continue;
            }
            if let Some(id) = name.strip_suffix(".collections.json") {
                if let (Ok(id), Ok(list)) = (
                    id.parse::<ProfileId>(),
                    serde_json::from_str::<Vec<CollectionBackup>>(&text),
                ) {
                    collections.push((id, list));
                }
            } else if let Ok(p) = serde_json::from_str::<Profile>(&text) {
                db.save_profile(&p)?;
                n += 1;
            }
        }
        // After the profiles, which the collections belong to.
        let stamp = now();
        for (profile, list) in collections {
            if db.profile(&profile)?.is_none() {
                continue;
            }
            for c in list {
                let record = CollectionRecord {
                    id: c.id,
                    name: c.name,
                    query: c.query,
                    position: c.position,
                };
                db.save_collection(&profile, &record, &stamp)?;
            }
        }
        Ok(n)
    }

    pub(crate) fn backup_collections(&self, profile: &ProfileId) -> Result<()> {
        if !self.session_info().is_some_and(|s| s.kind.keeps_data()) {
            return Ok(());
        }
        let list: Vec<CollectionBackup> = self
            .with_db(|db| db.collections(profile))?
            .into_iter()
            .map(|c| CollectionBackup {
                id: c.id,
                name: c.name,
                query: c.query,
                position: c.position,
            })
            .collect();
        let path = self
            .profiles_dir()
            .join(format!("{profile}.collections.json"));
        if list.is_empty() {
            let _ = fs::remove_file(path);
            return Ok(());
        }
        fs::create_dir_all(self.profiles_dir())?;
        let json = serde_json::to_vec_pretty(&list).map_err(std::io::Error::other)?;
        write_atomic(&path, &json)?;
        Ok(())
    }

    /// Opens the library straight away for someone who is alone in it: one
    /// profile (guests aside) and no PIN.
    pub(crate) fn sign_in_if_alone(&self) -> Result<()> {
        let people: Vec<Profile> = self
            .with_db(|db| db.profiles())?
            .into_iter()
            .filter(|p| p.kind != ProfileKind::Guest)
            .collect();
        if let [only] = people.as_slice() {
            if !only.has_pin() {
                self.start_session(only)?;
            }
        }
        Ok(())
    }

    fn start_session(&self, p: &Profile) -> Result<()> {
        self.end_guest_visit()?;
        let mut p = p.clone();
        p.last_used = Some(now());
        self.with_db(|db| db.save_profile(&p))?;
        if p.kind != ProfileKind::Guest {
            self.profile_backup(&p)?;
        }
        self.set_session(Some(Session {
            id: p.id,
            kind: p.kind,
            within: p.allowed_folders.clone(),
        }));
        Ok(())
    }

    /// Forgets everything a guest did.
    fn end_guest_visit(&self) -> Result<()> {
        if let Some(s) = self.session_info() {
            if s.kind == ProfileKind::Guest {
                self.with_db(|db| db.clear_personal_data(&s.id))?;
            }
        }
        Ok(())
    }

    /// Refuses unless the owner is signed in.
    pub fn require_owner_profile(&self) -> Result<()> {
        self.require_owner().map(|_| ())
    }

    pub(crate) fn require_owner(&self) -> Result<Session> {
        match self.session_info() {
            None => Err(Error::SignedOut),
            Some(s) if s.kind.can_manage_profiles() => Ok(s),
            Some(_) => Err(Error::NotAllowed(
                "only the library's owner can manage profiles".into(),
            )),
        }
    }

    pub(crate) fn load_profile(&self, id: &ProfileId) -> Result<Profile> {
        self.with_db(|db| db.profile(id))?
            .ok_or_else(|| Error::InvalidInput("that profile no longer exists".into()))
    }

    // ---- Public API --------------------------------------------------------

    /// Everyone who uses this library, owner first.
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        self.with_db(|db| db.profiles())
    }

    /// The signed-in profile.
    pub fn current_profile(&self) -> Result<Option<Profile>> {
        match self.session_info() {
            Some(s) => self.with_db(|db| db.profile(&s.id)),
            None => Ok(None),
        }
    }

    /// Seconds until `id` may try a PIN again (0 = now).
    pub fn pin_wait(&self, id: &ProfileId) -> Result<u64> {
        let (_, until) = self.with_db(|db| db.pin_attempts(id))?;
        Ok((until - unix_now()).max(0) as u64)
    }

    /// Signs in as `id`. A profile with a PIN needs it; wrong PINs count
    /// towards a growing wait.
    pub fn sign_in(&self, id: &ProfileId, pin: Option<&str>) -> Result<Profile> {
        let p = self.load_profile(id)?;
        if let Some(hash) = &p.pin_hash {
            self.check_pin(&p.id, hash, pin.unwrap_or(""))?;
        }
        self.start_session(&p)?;
        self.load_profile(id)
    }

    /// Verifies a PIN, counting wrong ones.
    fn check_pin(&self, id: &ProfileId, hash: &str, pin: &str) -> Result<()> {
        let (failed, until) = self.with_db(|db| db.pin_attempts(id))?;
        let now = unix_now();
        if until > now {
            return Err(Error::PinLocked((until - now) as u64));
        }
        if verify_secret(pin, hash) {
            if failed > 0 {
                self.with_db(|db| db.set_pin_attempts(id, 0, 0))?;
            }
            return Ok(());
        }
        let failed = failed + 1;
        let wait = lockout_seconds(failed);
        self.with_db(|db| db.set_pin_attempts(id, failed, now + wait as i64))?;
        Err(Error::WrongPin {
            attempts_left: FREE_ATTEMPTS.saturating_sub(failed),
            wait_seconds: wait,
        })
    }

    /// Locks the library: back to the profile picker. A guest's reading is
    /// forgotten.
    pub fn sign_out(&self) -> Result<()> {
        self.end_guest_visit()?;
        self.set_session(None);
        Ok(())
    }

    /// Adds a profile (owner only). `pin` is optional.
    pub fn create_profile(
        &self,
        name: &str,
        colour: &str,
        kind: ProfileKind,
        pin: Option<&str>,
    ) -> Result<Profile> {
        self.require_owner()?;
        let name = validate_profile_name(name).map_err(|e| Error::InvalidInput(e.into()))?;
        check_colour(colour)?;
        let all = self.profiles()?;
        match kind {
            ProfileKind::Owner => {
                return Err(Error::InvalidInput(
                    "a library has one owner; add this person as a standard profile".into(),
                ))
            }
            ProfileKind::Guest if all.iter().any(|p| p.kind == ProfileKind::Guest) => {
                return Err(Error::InvalidInput(
                    "there is already a guest profile".into(),
                ))
            }
            _ => {}
        }
        check_name_free(&all, None, &name)?;
        let mut p = Profile::new(name, colour, kind, &now());
        if let Some(pin) = pin.filter(|p| !p.is_empty()) {
            if kind == ProfileKind::Guest {
                return Err(Error::InvalidInput(
                    "a guest profile cannot have a PIN".into(),
                ));
            }
            validate_pin(pin).map_err(pin_error)?;
            p.pin_hash = Some(protect(pin)?);
        }
        self.with_db(|db| db.save_profile(&p))?;
        self.profile_backup(&p)?;
        Ok(p)
    }

    /// Renames or recolours a profile, or changes its kind. People may
    /// change their own name and colour; the owner may change anyone's, and
    /// the kind of anyone but themselves.
    pub fn update_profile(
        &self,
        id: &ProfileId,
        name: &str,
        colour: &str,
        kind: ProfileKind,
    ) -> Result<Profile> {
        let me = self.session_info().ok_or(Error::SignedOut)?;
        let mut p = self.load_profile(id)?;
        let is_self = me.id == p.id;
        if !is_self && !me.kind.can_manage_profiles() {
            return Err(Error::NotAllowed(
                "only the library's owner can change other profiles".into(),
            ));
        }
        if kind != p.kind {
            if !me.kind.can_manage_profiles() || is_self {
                return Err(Error::NotAllowed(
                    "the profile type cannot be changed here".into(),
                ));
            }
            if kind == ProfileKind::Owner {
                return Err(Error::InvalidInput("a library has one owner".into()));
            }
            if kind == ProfileKind::Guest {
                return Err(Error::InvalidInput(
                    "an existing profile cannot become a guest".into(),
                ));
            }
        }
        let name = validate_profile_name(name).map_err(|e| Error::InvalidInput(e.into()))?;
        check_colour(colour)?;
        if name != p.name {
            check_name_free(&self.profiles()?, Some(&p.id), &name)?;
            self.rename_notes_folder(&p, &name)?;
        }
        p.name = name;
        p.colour = colour.to_owned();
        p.kind = kind;
        self.with_db(|db| db.save_profile(&p))?;
        self.profile_backup(&p)?;
        if is_self {
            self.set_session(Some(Session {
                id: p.id,
                kind: p.kind,
                within: p.allowed_folders.clone(),
            }));
        }
        Ok(p)
    }

    /// `Notes/<old name>` becomes `Notes/<new name>`, and notebook paths
    /// follow.
    pub(crate) fn rename_notes_folder(&self, p: &Profile, new_name: &str) -> Result<()> {
        let old_dir = crate::reading::notes_folder_name(&p.name);
        let new_dir = crate::reading::notes_folder_name(new_name);
        if old_dir == new_dir {
            return Ok(());
        }
        // Libraries from before names were checked this way may have two
        // profiles sharing one folder. Then the folder stays for the other
        // profile; this profile's notebooks stay where they are and new ones
        // go to the new folder.
        if self.notes_folder_shared(p)? {
            return Ok(());
        }
        self.rename_feeds_folder(&p.name, new_name)?;
        let from = self.layout().notes_dir().join(&old_dir);
        let to = self.layout().notes_dir().join(&new_dir);
        if from.is_dir() {
            if to.exists() {
                return Err(Error::NameTaken(format!("Notes/{new_dir}")));
            }
            fs::rename(&from, &to)?;
        }
        self.with_db(|db| {
            db.move_notebook_paths(
                &p.id,
                &format!("Notes/{old_dir}/"),
                &format!("Notes/{new_dir}/"),
            )
        })?;
        Ok(())
    }

    /// Sets, changes or removes (`new_pin` = None) a PIN. Changing your own
    /// PIN needs the current one; the owner can reset anyone else's. The
    /// owner gets a new recovery code whenever they set a PIN.
    pub fn set_pin(
        &self,
        id: &ProfileId,
        current_pin: Option<&str>,
        new_pin: Option<&str>,
    ) -> Result<PinChange> {
        let me = self.session_info().ok_or(Error::SignedOut)?;
        let mut p = self.load_profile(id)?;
        let is_self = me.id == p.id;
        if !is_self && !me.kind.can_manage_profiles() {
            return Err(Error::NotAllowed(
                "only the library's owner can change someone else's PIN".into(),
            ));
        }
        if is_self {
            if let Some(hash) = p.pin_hash.clone() {
                self.check_pin(&p.id, &hash, current_pin.unwrap_or(""))?;
            }
        }
        if p.kind == ProfileKind::Guest && new_pin.is_some() {
            return Err(Error::InvalidInput(
                "a guest profile cannot have a PIN".into(),
            ));
        }
        let mut change = PinChange {
            recovery_code: None,
        };
        match new_pin {
            Some(pin) => {
                validate_pin(pin).map_err(pin_error)?;
                p.pin_hash = Some(protect(pin)?);
                if p.kind == ProfileKind::Owner {
                    let code = new_recovery_code();
                    p.recovery_hash = Some(protect(&normalise_code(&code))?);
                    change.recovery_code = Some(code);
                }
            }
            None => {
                p.pin_hash = None;
                p.recovery_hash = None;
            }
        }
        self.with_db(|db| {
            db.save_profile(&p)?;
            db.set_pin_attempts(&p.id, 0, 0)
        })?;
        self.profile_backup(&p)?;
        Ok(change)
    }

    /// The owner forgot their PIN: the recovery code sets a new one and
    /// signs them in. Returns a new recovery code (each code works once).
    pub fn recover_owner(&self, recovery_code: &str, new_pin: &str) -> Result<String> {
        validate_pin(new_pin).map_err(pin_error)?;
        let owner = self
            .profiles()?
            .into_iter()
            .find(|p| p.kind == ProfileKind::Owner)
            .ok_or_else(|| Error::InvalidInput("this library has no owner".into()))?;
        let Some(hash) = owner.recovery_hash.clone() else {
            return Err(Error::InvalidInput(
                "no recovery code was set for this library".into(),
            ));
        };
        let (failed, until) = self.with_db(|db| db.pin_attempts(&owner.id))?;
        let now_s = unix_now();
        if until > now_s {
            return Err(Error::PinLocked((until - now_s) as u64));
        }
        if !verify_secret(&normalise_code(recovery_code), &hash) {
            let failed = failed + 1;
            let wait = lockout_seconds(failed);
            self.with_db(|db| db.set_pin_attempts(&owner.id, failed, now_s + wait as i64))?;
            return Err(Error::InvalidInput(
                "that recovery code is not right".into(),
            ));
        }
        let mut p = owner;
        p.pin_hash = Some(protect(new_pin)?);
        let code = new_recovery_code();
        p.recovery_hash = Some(protect(&normalise_code(&code))?);
        self.with_db(|db| {
            db.save_profile(&p)?;
            db.set_pin_attempts(&p.id, 0, 0)
        })?;
        self.profile_backup(&p)?;
        self.start_session(&p)?;
        Ok(code)
    }

    /// Removes a profile with all its reading data (owner only). Its
    /// notebooks folder goes to the system trash, so it can be restored.
    pub fn delete_profile(&self, id: &ProfileId) -> Result<()> {
        let me = self.require_owner()?;
        let p = self.load_profile(id)?;
        if p.id == me.id || p.kind == ProfileKind::Owner {
            return Err(Error::InvalidInput(
                "the owner's profile cannot be removed".into(),
            ));
        }
        self.with_db(|db| db.delete_profile(&p.id))?;
        let _ = fs::remove_file(self.profiles_dir().join(format!("{}.json", p.id)));
        let _ = fs::remove_file(
            self.profiles_dir()
                .join(format!("{}.collections.json", p.id)),
        );
        let annotations = self
            .layout()
            .data_dir()
            .join("annotations")
            .join(p.id.to_string());
        let _ = fs::remove_dir_all(annotations);
        let notes = self
            .layout()
            .notes_dir()
            .join(crate::reading::notes_folder_name(&p.name));
        // Never trash a folder that another profile also uses.
        if notes.is_dir() && !self.notes_folder_shared(&p)? {
            trash::delete(&notes).map_err(|e| Error::Trash(e.to_string()))?;
        }
        let feeds = self
            .layout()
            .feeds_dir()
            .join(crate::reading::notes_folder_name(&p.name));
        if feeds.is_dir() && !self.notes_folder_shared(&p)? {
            trash::delete(&feeds).map_err(|e| Error::Trash(e.to_string()))?;
        }
        Ok(())
    }

    /// Whether another profile's notes folder is the same as `p`'s.
    fn notes_folder_shared(&self, p: &Profile) -> Result<bool> {
        Ok(self
            .profiles()?
            .iter()
            .any(|o| o.id != p.id && crate::reading::same_notes_folder(&o.name, &p.name)))
    }

    /// Which folders (relative to `Books/`) a Kids profile may open.
    pub fn set_allowed_folders(&self, id: &ProfileId, folders: Vec<String>) -> Result<Profile> {
        self.require_owner()?;
        let mut p = self.load_profile(id)?;
        let mut folders: Vec<String> = folders
            .into_iter()
            .map(|f| f.trim_matches('/').to_owned())
            .collect();
        folders.sort();
        folders.dedup();
        p.allowed_folders = folders;
        self.with_db(|db| db.save_profile(&p))?;
        self.profile_backup(&p)?;
        Ok(p)
    }

    /// Keeps Kids profiles' allowed folders pointing at the same folders
    /// after `from` (relative to `Books/`) was renamed or moved to `to`, or
    /// trashed (`to` = None: the folder is dropped from the list).
    pub(crate) fn follow_folder_change(&self, from: &str, to: Option<&str>) -> Result<()> {
        let from = from.trim_matches('/');
        for mut p in self.profiles()? {
            let mut changed = false;
            let mut folders = Vec::with_capacity(p.allowed_folders.len());
            for f in &p.allowed_folders {
                let rest = if f == from {
                    Some("")
                } else {
                    f.strip_prefix(from).filter(|r| r.starts_with('/'))
                };
                match (rest, to) {
                    (None, _) => folders.push(f.clone()),
                    (Some(rest), Some(to)) => {
                        folders.push(format!("{}{rest}", to.trim_matches('/')));
                        changed = true;
                    }
                    (Some(_), None) => changed = true,
                }
            }
            if changed {
                folders.sort();
                folders.dedup();
                p.allowed_folders = folders;
                self.with_db(|db| db.save_profile(&p))?;
                self.profile_backup(&p)?;
            }
        }
        Ok(())
    }

    /// Saves the signed-in profile's interface preferences (JSON).
    pub fn set_prefs(&self, prefs: &str) -> Result<()> {
        serde_json::from_str::<serde_json::Value>(prefs)
            .map_err(|_| Error::InvalidInput("the preferences are not valid JSON".into()))?;
        let id = self.profile()?;
        let mut p = self.load_profile(&id)?;
        p.prefs = prefs.to_owned();
        self.with_db(|db| db.save_profile(&p))?;
        if p.kind != ProfileKind::Guest {
            self.profile_backup(&p)?;
        }
        Ok(())
    }

    // ---- Smart collections ---------------------------------------------------

    pub fn collections(&self) -> Result<Vec<Collection>> {
        let profile = self.profile()?;
        Ok(self
            .with_db(|db| db.collections(&profile))?
            .into_iter()
            .filter_map(|c| {
                Some(Collection {
                    query: serde_json::from_str(&c.query).ok()?,
                    id: c.id,
                    name: c.name,
                })
            })
            .collect())
    }

    /// Adds (empty `id`) or updates a smart collection.
    pub fn save_collection(&self, id: &str, name: &str, query: &BookQuery) -> Result<Collection> {
        let profile = self.profile()?;
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err(Error::InvalidInput("a collection needs a name".into()));
        }
        let existing = self.with_db(|db| db.collections(&profile))?;
        let (id, position) = if id.is_empty() {
            (
                uuid::Uuid::new_v4().to_string(),
                existing.iter().map(|c| c.position + 1).max().unwrap_or(0),
            )
        } else {
            let pos = existing
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.position)
                .ok_or_else(|| Error::InvalidInput("that collection no longer exists".into()))?;
            (id.to_owned(), pos)
        };
        let record = CollectionRecord {
            id: id.clone(),
            name: name.clone(),
            query: serde_json::to_string(query).map_err(std::io::Error::other)?,
            position,
        };
        self.with_db(|db| db.save_collection(&profile, &record, &now()))?;
        self.backup_collections(&profile)?;
        Ok(Collection {
            id,
            name,
            query: query.clone(),
        })
    }

    pub fn delete_collection(&self, id: &str) -> Result<()> {
        let profile = self.profile()?;
        self.with_db(|db| db.delete_collection(&profile, id))?;
        self.backup_collections(&profile)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::{BookQuery, ProfileKind};

    #[test]
    fn a_lone_owner_is_signed_in_and_others_must_choose() {
        let (dir, lib) = library();
        let owner = lib.current_profile().unwrap().expect("signed in");
        assert_eq!(owner.kind, ProfileKind::Owner);

        let jane = lib
            .create_profile("Jane Smith", "teal", ProfileKind::Standard, None)
            .unwrap();
        let root = lib.layout().root().to_path_buf();
        lib.close().unwrap();

        let lib = Library::open(&root, OpenOptions::default()).unwrap();
        assert!(
            lib.current_profile().unwrap().is_none(),
            "two people: ask who"
        );
        assert!(matches!(lib.profile(), Err(Error::SignedOut)));
        lib.sign_in(&jane.id, None).unwrap();
        assert_eq!(lib.profile().unwrap(), jane.id);
        drop(dir);
    }

    #[test]
    fn pins_are_checked_and_wrong_ones_add_a_wait() {
        let (_d, lib) = library();
        let owner = lib.profile().unwrap();
        let kid = lib
            .create_profile("John Smith", "blue", ProfileKind::Kids, Some("480715"))
            .unwrap();
        assert!(kid.has_pin());
        assert!(!kid.pin_hash.as_deref().unwrap().contains("480715"));
        lib.sign_out().unwrap();

        for i in 1..=4 {
            match lib.sign_in(&kid.id, Some("000000")) {
                Err(Error::WrongPin {
                    attempts_left,
                    wait_seconds: 0,
                }) => assert_eq!(attempts_left, 5 - i),
                other => panic!("{other:?}"),
            }
        }
        assert!(matches!(
            lib.sign_in(&kid.id, Some("000000")),
            Err(Error::WrongPin {
                wait_seconds: 30,
                ..
            })
        ));
        // Even the right PIN waits.
        assert!(matches!(
            lib.sign_in(&kid.id, Some("480715")),
            Err(Error::PinLocked(_))
        ));
        assert!(lib.pin_wait(&kid.id).unwrap() > 0);

        // The owner has no PIN and resets the kid's.
        lib.sign_in(&owner, None).unwrap();
        lib.set_pin(&kid.id, None, Some("902364")).unwrap();
        lib.sign_in(&kid.id, Some("902364")).unwrap();
        assert_eq!(lib.profile().unwrap(), kid.id);
    }

    #[test]
    fn owner_recovery_code_resets_a_forgotten_pin() {
        let (_d, lib) = library();
        let owner = lib.profile().unwrap();
        let change = lib.set_pin(&owner, None, Some("375920")).unwrap();
        let code = change.recovery_code.expect("owner gets a code");
        assert_eq!(code.len(), 19);
        lib.sign_out().unwrap();
        assert!(lib.recover_owner("WRONG-CODE", "618204").is_err());
        let next = lib.recover_owner(&code.to_lowercase(), "618204").unwrap();
        assert_ne!(next, code);
        assert_eq!(lib.profile().unwrap(), owner);
        lib.sign_out().unwrap();
        assert!(lib.sign_in(&owner, Some("375920")).is_err());
        lib.sign_in(&owner, Some("618204")).unwrap();
        // Changing your own PIN needs the current one.
        assert!(lib.set_pin(&owner, Some("740258"), Some("529163")).is_err());
    }

    #[test]
    fn kids_see_only_allowed_folders_and_cannot_edit() {
        let (_d, lib) = library();
        let books = lib.layout().books_dir();
        md_book(&books, "Children/a.md", "Gruffalo");
        md_book(&books, "Grown-ups/b.md", "War and Peace");
        lib.scan(&NoProgress).unwrap();
        let all = lib.books(&BookQuery::default()).unwrap();
        let war = all
            .iter()
            .find(|b| b.metadata.title == "War and Peace")
            .unwrap()
            .clone();

        let kid = lib
            .create_profile("John Smith", "blue", ProfileKind::Kids, None)
            .unwrap();
        lib.set_allowed_folders(&kid.id, vec!["Children".into()])
            .unwrap();
        lib.sign_in(&kid.id, None).unwrap();

        let titles: Vec<_> = lib
            .books(&BookQuery::default())
            .unwrap()
            .into_iter()
            .map(|b| b.metadata.title)
            .collect();
        assert_eq!(titles, ["Gruffalo"]);
        assert_eq!(lib.facets().unwrap().total, 1);
        assert!(matches!(lib.book(&war.id), Err(Error::BookNotFound)));
        assert!(!lib.may_open(&war.rel_path));
        assert!(lib.may_open("Books/Children/a.md"));
        assert!(!lib.may_open(".library-data/library.db"));
        assert!(matches!(
            lib.create_folder("", "Mine"),
            Err(Error::NotAllowed(_))
        ));
        assert!(matches!(
            lib.trash_books(std::slice::from_ref(&war.id)),
            Err(Error::NotAllowed(_))
        ));
    }

    #[test]
    fn guests_leave_nothing_behind() {
        let (_d, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        let guest = lib
            .create_profile("Guest", "graphite", ProfileKind::Guest, None)
            .unwrap();
        lib.sign_in(&guest.id, None).unwrap();
        lib.save_position(&book.id, r#"{"type":"text","offset":1}"#, 0.5)
            .unwrap();
        assert!(lib.position(&book.id).unwrap().is_some());
        lib.sign_out().unwrap();
        lib.sign_in(&guest.id, None).unwrap();
        assert!(lib.position(&book.id).unwrap().is_none());
        assert!(!lib
            .layout()
            .data_dir()
            .join("annotations")
            .join(guest.id.to_string())
            .exists());
    }

    #[test]
    fn profiles_and_personal_data_survive_a_rebuild() {
        let (_d, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        let jane = lib
            .create_profile("Jane Smith", "teal", ProfileKind::Standard, Some("480715"))
            .unwrap();
        lib.sign_in(&jane.id, Some("480715")).unwrap();
        let mut state = book.user.clone();
        state.rating = 4;
        state.favorite = true;
        lib.set_user_state(&book.id, &state).unwrap();
        lib.save_position(&book.id, r#"{"type":"text","offset":7}"#, 0.3)
            .unwrap();
        lib.save_collection(
            "",
            "Favourites",
            &BookQuery {
                favorites_only: true,
                ..Default::default()
            },
        )
        .unwrap();

        lib.rebuild_index(&NoProgress).unwrap();
        let names: Vec<_> = lib
            .profiles()
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Jane Smith".to_owned()));
        let back = lib.book(&book.id).unwrap();
        assert_eq!(back.user.rating, 4);
        assert!(back.user.favorite);
        assert_eq!(
            lib.position(&book.id).unwrap().as_deref(),
            Some(r#"{"type":"text","offset":7}"#)
        );
        assert_eq!(lib.collections().unwrap()[0].name, "Favourites");
        lib.sign_out().unwrap();
        assert!(lib.sign_in(&jane.id, Some("480715")).is_ok(), "PIN kept");
    }

    #[test]
    fn renaming_a_profile_moves_its_notes() {
        let (_d, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        let me = lib.current_profile().unwrap().unwrap();
        let nb = lib.notebook(&book.id).unwrap();
        lib.update_profile(&me.id, "Jane Smith", "violet", ProfileKind::Owner)
            .unwrap();
        let moved = lib.notebook(&book.id).unwrap();
        assert!(
            moved.rel_path.starts_with("Notes/Jane Smith/"),
            "{}",
            moved.rel_path
        );
        assert_eq!(moved.content, nb.content);
    }

    #[test]
    fn names_that_share_a_notes_folder_are_refused() {
        let (_d, lib) = library();
        let sam = lib
            .create_profile("Sam", "teal", ProfileKind::Standard, None)
            .unwrap();
        for clash in ["sam", "Sam.", "SAM..", "Sam "] {
            assert!(
                lib.create_profile(clash, "blue", ProfileKind::Standard, None)
                    .is_err(),
                "{clash} should be refused"
            );
        }
        lib.create_profile("Élise", "blue", ProfileKind::Standard, None)
            .unwrap();
        assert!(lib
            .create_profile("élise", "blue", ProfileKind::Standard, None)
            .is_err());
        lib.create_profile("CON", "blue", ProfileKind::Standard, None)
            .unwrap();
        assert!(
            lib.create_profile("AUX", "blue", ProfileKind::Standard, None)
                .is_err(),
            "both would be Notes/Untitled"
        );
        let other = lib
            .create_profile("Samuel", "blue", ProfileKind::Standard, None)
            .unwrap();
        assert!(lib
            .update_profile(&other.id, "sam.", "blue", ProfileKind::Standard)
            .is_err());
        // Renaming yourself to a different case of your own name is fine.
        lib.update_profile(&sam.id, "SAM", "teal", ProfileKind::Standard)
            .unwrap();
    }

    #[test]
    fn deleting_a_profile_keeps_a_notes_folder_another_one_uses() {
        let (_d, lib) = library();
        let sam = lib
            .create_profile("Sam", "teal", ProfileKind::Standard, None)
            .unwrap();
        // An older library could have "Sam." too, sharing Notes/Sam.
        let twin = libreri_core::Profile::new("Sam.", "blue", ProfileKind::Standard, "now");
        lib.with_db(|db| db.save_profile(&twin)).unwrap();
        let dir = lib.layout().notes_dir().join("Sam");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mine.md"), "Sam's notes").unwrap();

        // Renaming the twin leaves the shared folder where it is.
        lib.update_profile(&twin.id, "Sammy", "blue", ProfileKind::Standard)
            .unwrap();
        assert!(dir.join("mine.md").is_file());

        let twin = libreri_core::Profile::new("Sam.", "blue", ProfileKind::Standard, "now");
        lib.with_db(|db| db.save_profile(&twin)).unwrap();
        lib.delete_profile(&twin.id).unwrap();
        assert!(dir.join("mine.md").is_file(), "Sam's folder must stay");
        assert!(lib.load_profile(&sam.id).is_ok());
    }

    #[test]
    fn collections_round_trip() {
        let (_d, lib) = library();
        let q = BookQuery {
            search: Some("physics".into()),
            ..Default::default()
        };
        let c = lib.save_collection("", "Physics", &q).unwrap();
        let renamed = lib.save_collection(&c.id, "Physics books", &q).unwrap();
        assert_eq!(lib.collections().unwrap(), vec![renamed]);
        lib.delete_collection(&c.id).unwrap();
        assert!(lib.collections().unwrap().is_empty());
    }
}
