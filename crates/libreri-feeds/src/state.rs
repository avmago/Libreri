//! A profile's feeds: folders (categories and subcategories), feeds, the
//! items that came in, and settings. Kept as `Feeds/<profile>/feeds.json`.

use crate::opml::Node;
use crate::parse::{FeedEntry, Parsed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Items remembered after they were deleted, so they do not come back.
const GONE_DAYS: i64 = 400;
const GONE_MAX: usize = 50_000;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FeedSettings {
    /// Check for new items every this many minutes while Libreri is open
    /// (0: only when asked).
    pub refresh_minutes: u32,
    /// Items not downloaded are removed after this many days.
    pub keep_days: u32,
}

impl Default for FeedSettings {
    fn default() -> Self {
        Self {
            refresh_minutes: 60,
            keep_days: 30,
        }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedFolder {
    pub id: String,
    pub name: String,
    /// The folder it is in (none: top level).
    pub parent: Option<String>,
    /// Download new items of every feed in it (and its subfolders).
    #[serde(default)]
    pub auto_download: bool,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feed {
    pub id: String,
    pub url: String,
    pub title: String,
    /// The site's own address.
    pub site: Option<String>,
    pub folder: Option<String>,
    #[serde(default)]
    pub auto_download: bool,
    /// For "not modified" answers.
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// When it was last read (RFC 3339), and why that failed.
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// Podcasts: who makes it, its artwork's address, and a small copy of
    /// the artwork (a data URL, so it shows offline).
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub artwork: Option<String>,
    /// Podcasts: play at this speed (none: the usual).
    #[serde(default)]
    pub speed: Option<f64>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    pub id: String,
    pub feed: String,
    /// The feed's title when it came in (kept if the feed is removed).
    pub source: String,
    #[serde(flatten)]
    pub entry: FeedEntry,
    /// When it came in (RFC 3339).
    pub found_at: String,
    #[serde(default)]
    pub read: bool,
    /// The downloaded file (library-relative, under `Feeds/`).
    pub file: Option<String>,
    /// The book it became when added to the library.
    pub book: Option<String>,
    pub download_error: Option<String>,
    /// Podcasts: where listening stopped (seconds), and whether it was
    /// heard to the end.
    #[serde(default)]
    pub position: Option<f64>,
    #[serde(default)]
    pub played: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub version: u32,
    pub folders: Vec<FeedFolder>,
    pub feeds: Vec<Feed>,
    pub items: Vec<FeedItem>,
    /// Items deleted or expired ("<feed id> <key>" → when), so they do not
    /// come back while the feed still lists them.
    pub gone: BTreeMap<String, String>,
    pub settings: FeedSettings,
    /// Counter for new ids.
    pub next_id: u64,
    /// Podcasts: episodes to play next, in order.
    pub queue: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            folders: Vec::new(),
            feeds: Vec::new(),
            items: Vec::new(),
            gone: BTreeMap::new(),
            settings: FeedSettings::default(),
            next_id: 1,
            queue: Vec::new(),
        }
    }
}

fn gone_key(feed: &str, key: &str) -> String {
    format!("{feed} {key}")
}

/// A short stable id for an item (FNV-1a of feed and key).
fn item_id(feed: &str, key: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in feed.bytes().chain([0]).chain(key.bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("i{h:016x}")
}

pub type Now = chrono::DateTime<chrono::Utc>;

impl State {
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("the feeds file could not be read: {e}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    fn new_id(&mut self, prefix: char) -> String {
        let id = format!("{prefix}{}", self.next_id);
        self.next_id += 1;
        id
    }

    pub fn folder(&self, id: &str) -> Option<&FeedFolder> {
        self.folders.iter().find(|f| f.id == id)
    }

    pub fn feed(&self, id: &str) -> Option<&Feed> {
        self.feeds.iter().find(|f| f.id == id)
    }

    pub fn item(&self, id: &str) -> Option<&FeedItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn item_mut(&mut self, id: &str) -> Option<&mut FeedItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// The folder's names from the top ("Science", "Physics").
    pub fn folder_path(&self, id: Option<&str>) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = id.and_then(|i| self.folder(i));
        let mut seen = HashSet::new();
        while let Some(f) = cur {
            if !seen.insert(f.id.clone()) {
                break;
            }
            out.push(f.name.clone());
            cur = f.parent.as_deref().and_then(|p| self.folder(p));
        }
        out.reverse();
        out
    }

    /// The folder and every folder inside it.
    pub fn folder_and_inside(&self, id: &str) -> HashSet<String> {
        let mut out: HashSet<String> = HashSet::from([id.to_owned()]);
        loop {
            let before = out.len();
            for f in &self.folders {
                if f.parent.as_ref().is_some_and(|p| out.contains(p)) {
                    out.insert(f.id.clone());
                }
            }
            if out.len() == before {
                return out;
            }
        }
    }

    fn check_name(name: &str) -> Result<String, String> {
        let n = name.trim();
        if n.is_empty() {
            return Err("give the folder a name".into());
        }
        if n.chars().count() > 80 {
            return Err("the name is too long".into());
        }
        Ok(n.to_owned())
    }

    /// Adds a folder, or finds the one with that name in `parent`.
    pub fn add_folder(&mut self, name: &str, parent: Option<&str>) -> Result<String, String> {
        let name = Self::check_name(name)?;
        if parent.is_some_and(|p| self.folder(p).is_none()) {
            return Err("that folder no longer exists".into());
        }
        if let Some(f) = self
            .folders
            .iter()
            .find(|f| f.parent.as_deref() == parent && f.name.eq_ignore_ascii_case(&name))
        {
            return Ok(f.id.clone());
        }
        let id = self.new_id('f');
        self.folders.push(FeedFolder {
            id: id.clone(),
            name,
            parent: parent.map(str::to_owned),
            auto_download: false,
        });
        Ok(id)
    }

    /// Adds the folders of a path ("arXiv", "Computer Science").
    pub fn add_folder_path(
        &mut self,
        names: &[&str],
        parent: Option<&str>,
    ) -> Result<Option<String>, String> {
        let mut cur = parent.map(str::to_owned);
        for n in names {
            cur = Some(self.add_folder(n, cur.as_deref())?);
        }
        Ok(cur)
    }

    pub fn change_folder(
        &mut self,
        id: &str,
        name: Option<&str>,
        parent: Option<Option<&str>>,
        auto_download: Option<bool>,
    ) -> Result<(), String> {
        let name = name.map(Self::check_name).transpose()?;
        if let Some(Some(p)) = parent {
            if self.folder(p).is_none() {
                return Err("that folder no longer exists".into());
            }
            if self.folder_and_inside(id).contains(p) {
                return Err("a folder cannot go inside itself".into());
            }
        }
        let f = self
            .folders
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or("that folder no longer exists")?;
        if let Some(n) = name {
            f.name = n;
        }
        if let Some(p) = parent {
            f.parent = p.map(str::to_owned);
        }
        if let Some(a) = auto_download {
            f.auto_download = a;
        }
        Ok(())
    }

    /// Removes a folder; what was in it moves up a level.
    pub fn remove_folder(&mut self, id: &str) -> Result<(), String> {
        let parent = self
            .folder(id)
            .ok_or("that folder no longer exists")?
            .parent
            .clone();
        for f in &mut self.folders {
            if f.parent.as_deref() == Some(id) {
                f.parent = parent.clone();
            }
        }
        for f in &mut self.feeds {
            if f.folder.as_deref() == Some(id) {
                f.folder = parent.clone();
            }
        }
        self.folders.retain(|f| f.id != id);
        Ok(())
    }

    /// Adds a feed (its address must be new).
    pub fn add_feed(
        &mut self,
        url: &str,
        title: &str,
        site: Option<String>,
        folder: Option<&str>,
    ) -> Result<String, String> {
        if let Some(f) = self.feeds.iter().find(|f| f.url == url) {
            return Err(format!("you already follow this feed (“{}”)", f.title));
        }
        if folder.is_some_and(|p| self.folder(p).is_none()) {
            return Err("that folder no longer exists".into());
        }
        let id = self.new_id('s');
        self.feeds.push(Feed {
            id: id.clone(),
            url: url.to_owned(),
            title: if title.trim().is_empty() {
                url.to_owned()
            } else {
                title.trim().to_owned()
            },
            site,
            folder: folder.map(str::to_owned),
            auto_download: false,
            etag: None,
            last_modified: None,
            checked_at: None,
            error: None,
            author: None,
            image: None,
            artwork: None,
            speed: None,
        });
        Ok(id)
    }

    pub fn change_feed(
        &mut self,
        id: &str,
        title: Option<&str>,
        folder: Option<Option<&str>>,
        auto_download: Option<bool>,
    ) -> Result<(), String> {
        if let Some(Some(p)) = folder {
            if self.folder(p).is_none() {
                return Err("that folder no longer exists".into());
            }
        }
        let f = self
            .feeds
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or("that feed is no longer followed")?;
        if let Some(t) = title.map(str::trim).filter(|t| !t.is_empty()) {
            f.title = t.to_owned();
        }
        if let Some(p) = folder {
            f.folder = p.map(str::to_owned);
        }
        if let Some(a) = auto_download {
            f.auto_download = a;
        }
        Ok(())
    }

    /// Stops following a feed. Its items go too, except those downloaded
    /// or added to the library.
    pub fn remove_feed(&mut self, id: &str) {
        self.feeds.retain(|f| f.id != id);
        self.items
            .retain(|i| i.feed != id || i.file.is_some() || i.book.is_some());
        let items: std::collections::HashSet<&str> =
            self.items.iter().map(|i| i.id.as_str()).collect();
        self.queue.retain(|q| items.contains(q.as_str()));
    }

    /// Adds a feed's new entries; returns the new items' ids.
    pub fn merge(&mut self, feed_id: &str, parsed: &Parsed, now: Now) -> Vec<String> {
        let Some(feed) = self.feeds.iter_mut().find(|f| f.id == feed_id) else {
            return Vec::new();
        };
        if feed.title == feed.url && !parsed.title.is_empty() {
            feed.title = parsed.title.clone();
        }
        if feed.site.is_none() {
            feed.site = parsed.site.clone();
        }
        if parsed.author.is_some() {
            feed.author = parsed.author.clone();
        }
        if parsed.image.is_some() && feed.image != parsed.image {
            feed.image = parsed.image.clone();
            // A new picture: its small copy is made again.
            feed.artwork = None;
        }
        let source = feed.title.clone();
        let have: HashSet<String> = self
            .items
            .iter()
            .filter(|i| i.feed == feed_id)
            .map(|i| i.entry.key.clone())
            .collect();
        let found_at = now.to_rfc3339();
        let mut added = Vec::new();
        let mut seen = HashSet::new();
        for e in &parsed.entries {
            if have.contains(&e.key)
                || !seen.insert(e.key.clone())
                || self.gone.contains_key(&gone_key(feed_id, &e.key))
            {
                continue;
            }
            let id = item_id(feed_id, &e.key);
            added.push(id.clone());
            self.items.push(FeedItem {
                id,
                feed: feed_id.to_owned(),
                source: source.clone(),
                entry: e.clone(),
                found_at: found_at.clone(),
                read: false,
                file: None,
                book: None,
                download_error: None,
                position: None,
                played: false,
            });
        }
        added
    }

    /// Removes items older than the settings keep (unless downloaded or
    /// in the library), and old memories of deleted items.
    pub fn prune(&mut self, now: Now) {
        let keep = chrono::Duration::days(i64::from(self.settings.keep_days.max(1)));
        let mut gone = Vec::new();
        self.items.retain(|i| {
            let old = chrono::DateTime::parse_from_rfc3339(&i.found_at)
                .is_ok_and(|d| now.signed_duration_since(d) > keep);
            let started = i.position.is_some_and(|p| p > 0.0) && !i.played;
            let drop = old && i.file.is_none() && i.book.is_none() && !started;
            if drop {
                gone.push(gone_key(&i.feed, &i.entry.key));
            }
            !drop
        });
        let when = now.to_rfc3339();
        for g in gone {
            self.gone.insert(g, when.clone());
        }
        let limit = chrono::Duration::days(GONE_DAYS);
        self.gone.retain(|_, d| {
            chrono::DateTime::parse_from_rfc3339(d)
                .is_ok_and(|d| now.signed_duration_since(d) <= limit)
        });
        if self.gone.len() > GONE_MAX {
            let mut by_age: Vec<(String, String)> = self
                .gone
                .iter()
                .map(|(k, v)| (v.clone(), k.clone()))
                .collect();
            by_age.sort();
            for (_, k) in by_age.iter().take(self.gone.len() - GONE_MAX) {
                self.gone.remove(k);
            }
        }
    }

    /// Deletes an item: it will not come back. Returns its file, to delete.
    pub fn delete_item(&mut self, id: &str, now: Now) -> Option<String> {
        let pos = self.items.iter().position(|i| i.id == id)?;
        let item = self.items.remove(pos);
        self.queue.retain(|q| q != id);
        self.gone
            .insert(gone_key(&item.feed, &item.entry.key), now.to_rfc3339());
        item.file
    }

    /// Whether new items of this feed are downloaded by themselves (the
    /// feed's own setting, or a folder it is in).
    pub fn auto_download(&self, feed_id: &str) -> bool {
        let Some(feed) = self.feed(feed_id) else {
            return false;
        };
        if feed.auto_download {
            return true;
        }
        let mut cur = feed.folder.as_deref().and_then(|f| self.folder(f));
        let mut seen = HashSet::new();
        while let Some(f) = cur {
            if f.auto_download {
                return true;
            }
            if !seen.insert(f.id.as_str()) {
                break;
            }
            cur = f.parent.as_deref().and_then(|p| self.folder(p));
        }
        false
    }

    /// Adds the folders and feeds of an OPML file under `parent`. Returns
    /// (feeds added, feeds already followed).
    pub fn add_opml(&mut self, nodes: &[Node], parent: Option<&str>) -> Result<(u32, u32), String> {
        let (mut added, mut skipped) = (0, 0);
        for n in nodes {
            match n {
                Node::FeedFolder { name, children } => {
                    let id = self.add_folder(name, parent)?;
                    let (a, s) = self.add_opml(children, Some(&id))?;
                    added += a;
                    skipped += s;
                }
                Node::Feed { title, url, site } => {
                    if self.feeds.iter().any(|f| &f.url == url) {
                        skipped += 1;
                    } else {
                        self.add_feed(url, title, site.clone(), parent)?;
                        added += 1;
                    }
                }
            }
        }
        Ok((added, skipped))
    }

    /// The folders and feeds, for an OPML file.
    pub fn opml(&self) -> Vec<Node> {
        fn under(s: &State, parent: Option<&str>, depth: usize) -> Vec<Node> {
            if depth > 32 {
                return Vec::new();
            }
            let mut out: Vec<Node> = s
                .folders
                .iter()
                .filter(|f| f.parent.as_deref() == parent)
                .map(|f| Node::FeedFolder {
                    name: f.name.clone(),
                    children: under(s, Some(&f.id), depth + 1),
                })
                .collect();
            out.extend(
                s.feeds
                    .iter()
                    .filter(|f| f.folder.as_deref() == parent)
                    .map(|f| Node::Feed {
                        title: f.title.clone(),
                        url: f.url.clone(),
                        site: f.site.clone(),
                    }),
            );
            out
        }
        under(self, None, 0)
    }
}

impl State {
    /// Brings in another library's subscriptions (from an archive): folders
    /// by name, feeds by address, items by feed and key. Nothing here is
    /// lost; what is new gets ids of this state. `file` maps a download's
    /// path in the other library to its path here (None: not brought along).
    /// Returns how many feeds were added.
    pub fn absorb(&mut self, other: State, file: impl Fn(&str) -> Option<String>) -> u32 {
        let mut folders = std::collections::HashMap::new();
        for f in &other.folders {
            let path = other.folder_path(Some(&f.id));
            let names: Vec<&str> = path.iter().map(String::as_str).collect();
            if let Ok(Some(id)) = self.add_folder_path(&names, None) {
                if f.auto_download {
                    if let Some(mine) = self.folders.iter_mut().find(|x| x.id == id) {
                        mine.auto_download = true;
                    }
                }
                folders.insert(f.id.clone(), id);
            }
        }
        let mut feeds = std::collections::HashMap::new();
        let mut added = 0;
        for f in other.feeds {
            if let Some(mine) = self.feeds.iter().find(|x| x.url == f.url) {
                feeds.insert(f.id.clone(), mine.id.clone());
                continue;
            }
            let id = self.new_id('s');
            feeds.insert(f.id.clone(), id.clone());
            self.feeds.push(Feed {
                id,
                folder: f.folder.as_ref().and_then(|x| folders.get(x).cloned()),
                ..f
            });
            added += 1;
        }
        let mut items = std::collections::HashMap::new();
        for it in other.items {
            let Some(feed) = feeds.get(&it.feed).cloned() else {
                continue;
            };
            let id = item_id(&feed, &it.entry.key);
            items.insert(it.id.clone(), id.clone());
            let here = it.file.as_deref().and_then(&file);
            if let Some(mine) = self.items.iter_mut().find(|x| x.id == id) {
                mine.read |= it.read;
                mine.played |= it.played;
                if mine.position.unwrap_or(0.0) < it.position.unwrap_or(0.0) {
                    mine.position = it.position;
                }
                if mine.file.is_none() {
                    mine.file = here;
                }
                if mine.book.is_none() {
                    mine.book = it.book;
                }
                continue;
            }
            if self.gone.contains_key(&gone_key(&feed, &it.entry.key)) {
                continue;
            }
            self.items.push(FeedItem {
                id,
                feed,
                file: here,
                ..it
            });
        }
        for (k, when) in other.gone {
            let Some((feed, key)) = k.split_once(' ') else {
                continue;
            };
            if let Some(feed) = feeds.get(feed) {
                self.gone.entry(gone_key(feed, key)).or_insert(when);
            }
        }
        for q in other.queue {
            if let Some(id) = items.get(&q) {
                if !self.queue.contains(id) {
                    self.queue.push(id.clone());
                }
            }
        }
        added
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn absorbs_another_librarys_feeds_without_losing_anything() {
        let mut here = State::default();
        let sci = here.add_folder("Science", None).unwrap();
        let a = here
            .add_feed("https://a/rss", "A", None, Some(&sci))
            .unwrap();
        here.items.push(FeedItem {
            id: item_id(&a, "k1"),
            feed: a.clone(),
            source: "A".into(),
            entry: FeedEntry {
                key: "k1".into(),
                title: "One".into(),
                ..Default::default()
            },
            found_at: "2026-10-01T00:00:00Z".into(),
            read: false,
            file: None,
            book: None,
            download_error: None,
            position: Some(10.0),
            played: false,
        });
        let mut there = State {
            next_id: 50,
            ..Default::default()
        };
        let f = there.add_folder("Science", None).unwrap();
        let sub = there.add_folder("Physics", Some(&f)).unwrap();
        let a2 = there
            .add_feed("https://a/rss", "A", None, Some(&f))
            .unwrap();
        let b2 = there
            .add_feed("https://b/rss", "B", None, Some(&sub))
            .unwrap();
        for (feed, key) in [(&a2, "k1"), (&b2, "k2")] {
            there.items.push(FeedItem {
                id: item_id(feed, key),
                feed: feed.clone(),
                source: "x".into(),
                entry: FeedEntry {
                    key: key.into(),
                    title: key.into(),
                    ..Default::default()
                },
                found_at: "2026-10-01T00:00:00Z".into(),
                read: true,
                file: Some(format!("Feeds/Old/{key}.pdf")),
                book: None,
                download_error: None,
                position: Some(30.0),
                played: false,
            });
        }
        there.queue.push(item_id(&b2, "k2"));
        let added = here.absorb(there.clone(), |p| {
            Some(p.replace("Feeds/Old/", "Feeds/New/"))
        });
        assert_eq!(added, 1);
        assert_eq!(here.feeds.len(), 2);
        assert_eq!(
            here.folder_path(here.feeds[1].folder.as_deref()),
            ["Science", "Physics"]
        );
        let one = here.item(&item_id(&a, "k1")).unwrap();
        assert!(one.read);
        assert_eq!(one.position, Some(30.0));
        assert_eq!(one.file.as_deref(), Some("Feeds/New/k1.pdf"));
        let b = here.feeds[1].id.clone();
        assert_eq!(here.queue, vec![item_id(&b, "k2")]);
        // Again: nothing doubles.
        here.absorb(there, |p| Some(p.to_owned()));
        assert_eq!(
            (here.feeds.len(), here.items.len(), here.folders.len()),
            (2, 2, 2)
        );
    }

    use super::*;
    use chrono::TimeZone;

    fn entry(key: &str) -> FeedEntry {
        FeedEntry {
            key: key.into(),
            title: format!("Paper {key}"),
            ..Default::default()
        }
    }

    fn parsed(keys: &[&str]) -> Parsed {
        Parsed {
            title: "cs.AI updates".into(),
            site: Some("https://arxiv.org".into()),
            image: None,
            author: None,
            entries: keys.iter().map(|k| entry(k)).collect(),
        }
    }

    fn at(day: u32) -> Now {
        chrono::Utc
            .with_ymd_and_hms(2026, 9, day, 12, 0, 0)
            .unwrap()
    }

    #[test]
    fn merges_new_entries_once() {
        let mut s = State::default();
        let f = s
            .add_feed("https://rss.arxiv.org/rss/cs.AI", "", None, None)
            .unwrap();
        assert_eq!(s.merge(&f, &parsed(&["a", "b"]), at(1)).len(), 2);
        assert_eq!(s.feed(&f).unwrap().title, "cs.AI updates");
        assert_eq!(s.merge(&f, &parsed(&["b", "c", "c"]), at(2)).len(), 1);
        assert_eq!(s.items.len(), 3);
        // Deleted items do not come back.
        let id = s.items[0].id.clone();
        s.delete_item(&id, at(2));
        assert!(s.merge(&f, &parsed(&["a", "b", "c"]), at(3)).is_empty());
        assert!(s
            .add_feed("https://rss.arxiv.org/rss/cs.AI", "x", None, None)
            .is_err());
    }

    #[test]
    fn old_items_go_unless_kept() {
        let mut s = State::default();
        s.settings.keep_days = 7;
        let f = s.add_feed("https://x.org/feed", "X", None, None).unwrap();
        s.merge(&f, &parsed(&["old", "kept"]), at(1));
        s.merge(&f, &parsed(&["new"]), at(9));
        let kept = s.items.iter().position(|i| i.entry.key == "kept").unwrap();
        s.items[kept].file = Some("Feeds/Me/x.pdf".into());
        s.prune(at(10));
        let keys: Vec<_> = s.items.iter().map(|i| i.entry.key.as_str()).collect();
        assert_eq!(keys, ["kept", "new"]);
        // Expired items stay away while the feed lists them.
        assert!(s.merge(&f, &parsed(&["old"]), at(11)).is_empty());
    }

    #[test]
    fn folders_and_auto_download() {
        let mut s = State::default();
        let sci = s.add_folder("Science", None).unwrap();
        let phys = s
            .add_folder_path(&["Science", "Physics"], None)
            .unwrap()
            .unwrap();
        assert_eq!(s.add_folder("science", None).unwrap(), sci);
        let f = s
            .add_feed("https://x.org/hep", "hep-th", None, Some(&phys))
            .unwrap();
        assert!(!s.auto_download(&f));
        s.change_folder(&sci, None, None, Some(true)).unwrap();
        assert!(s.auto_download(&f));
        assert_eq!(s.folder_path(Some(&phys)), ["Science", "Physics"]);
        assert!(s
            .change_folder(&sci, None, Some(Some(&phys)), None)
            .is_err());
        s.remove_folder(&phys).unwrap();
        assert_eq!(s.feed(&f).unwrap().folder.as_deref(), Some(sci.as_str()));
    }

    #[test]
    fn opml_round_trip() {
        let mut s = State::default();
        let nodes = vec![Node::FeedFolder {
            name: "News".into(),
            children: vec![Node::Feed {
                title: "HN".into(),
                url: "https://news.ycombinator.com/rss".into(),
                site: None,
            }],
        }];
        assert_eq!(s.add_opml(&nodes, None).unwrap(), (1, 0));
        assert_eq!(s.add_opml(&nodes, None).unwrap(), (0, 1));
        assert_eq!(s.opml(), nodes);
        let json = s.to_json();
        assert_eq!(State::from_json(&json).unwrap(), s);
    }
}
