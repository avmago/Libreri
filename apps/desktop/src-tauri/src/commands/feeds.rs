//! Feeds (ADR 0027): the signed-in profile's subscriptions, the items that
//! came in, downloads into `Feeds/<profile>/`, and adding them to the
//! library. New items are looked for while Libreri is open.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::FeedsChanged;
use crate::state::AppState;
use libreri_core::{BookMetadata, ContentType};
use libreri_feeds::fetch::Download;
use libreri_feeds::sources::{ArxivGroup, Suggested};
use libreri_feeds::{FeedFolder, FeedItem, FeedSettings, State};
use libreri_library::Library;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

/// Feeds (papers, articles) or podcasts: each has its own folders, feeds
/// and items, in its own file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Space {
    Feeds,
    Podcasts,
}

impl Space {
    fn file(self) -> &'static str {
        match self {
            Space::Feeds => ".feeds.json",
            Space::Podcasts => ".podcasts.json",
        }
    }

    /// Downloads go under this folder of `Feeds/<profile>/` (podcasts in
    /// their own).
    fn downloads(self) -> Option<&'static str> {
        match self {
            Space::Feeds => None,
            Space::Podcasts => Some("Podcasts"),
        }
    }

    fn refreshing(self, state: &AppState) -> &std::sync::atomic::AtomicBool {
        match self {
            Space::Feeds => &state.feeds_refreshing,
            Space::Podcasts => &state.podcasts_refreshing,
        }
    }

    fn tag(self, id: &str) -> String {
        format!("{self:?}:{id}")
    }
}

fn load(lib: &Library, space: Space) -> AppResult<State> {
    match lib.read_feeds_file(space.file())? {
        Some(json) => State::from_json(&json).map_err(AppError::invalid),
        None => Ok(State::default()),
    }
}

fn save(lib: &Library, space: Space, s: &State) -> AppResult<()> {
    lib.write_feeds_file(space.file(), &s.to_json())?;
    Ok(())
}

/// Loads, changes and saves the state, one change at a time.
fn change<T>(
    state: &AppState,
    space: Space,
    f: impl FnOnce(&Library, &mut State) -> AppResult<T>,
) -> AppResult<T> {
    let lib = state.library()?;
    let _guard = state
        .feeds_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut s = load(&lib, space)?;
    let out = f(&lib, &mut s)?;
    save(&lib, space, &s)?;
    Ok(out)
}

fn read<T>(state: &AppState, space: Space, f: impl FnOnce(&State) -> T) -> AppResult<T> {
    let lib = state.library()?;
    let _guard = state
        .feeds_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(f(&load(&lib, space)?))
}

fn changed(app: &AppHandle, new_items: u32) {
    let _ = FeedsChanged { new_items }.emit(app);
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedDto {
    pub id: String,
    pub url: String,
    pub title: String,
    pub site: Option<String>,
    pub folder: Option<String>,
    pub auto_download: bool,
    /// Downloaded by itself because a folder it is in says so.
    pub auto_from_folder: bool,
    pub checked_at: Option<String>,
    pub error: Option<String>,
    pub unread: u32,
    pub total: u32,
    /// Podcasts: who makes it, its artwork (a data URL) and its speed.
    pub author: Option<String>,
    pub artwork: Option<String>,
    pub speed: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedsDto {
    pub folders: Vec<FeedFolder>,
    pub feeds: Vec<FeedDto>,
    pub settings: FeedSettings,
    pub unread: u32,
    pub total: u32,
    pub downloaded: u32,
    pub refreshing: bool,
    /// Items being downloaded now.
    pub downloading: Vec<String>,
    /// Podcasts: episodes to play next, in order.
    pub queue: Vec<String>,
    /// Podcasts: episodes started and not finished.
    pub in_progress: u32,
}

/// The folders and feeds, with counts.
#[tauri::command]
#[specta::specta]
pub async fn feeds_overview(app: AppHandle, space: Space) -> AppResult<FeedsDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let prefix = space.tag("");
        let downloading: Vec<String> = state
            .feed_downloads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|t| t.strip_prefix(&prefix).map(str::to_owned))
            .collect();
        let refreshing = space.refreshing(&state).load(Ordering::SeqCst);
        read(&state, space, |s| {
            let feeds = s
                .feeds
                .iter()
                .map(|f| {
                    let items = s.items.iter().filter(|i| i.feed == f.id);
                    let (mut unread, mut total) = (0, 0);
                    for i in items {
                        total += 1;
                        if !i.read {
                            unread += 1;
                        }
                    }
                    FeedDto {
                        id: f.id.clone(),
                        url: f.url.clone(),
                        title: f.title.clone(),
                        site: f.site.clone(),
                        folder: f.folder.clone(),
                        auto_download: f.auto_download,
                        auto_from_folder: !f.auto_download && s.auto_download(&f.id),
                        checked_at: f.checked_at.clone(),
                        error: f.error.clone(),
                        unread,
                        total,
                        author: f.author.clone(),
                        artwork: f.artwork.clone(),
                        speed: f.speed,
                    }
                })
                .collect();
            FeedsDto {
                folders: s.folders.clone(),
                feeds,
                settings: s.settings.clone(),
                unread: s.items.iter().filter(|i| !i.read).count() as u32,
                total: s.items.len() as u32,
                downloaded: s.items.iter().filter(|i| i.file.is_some()).count() as u32,
                refreshing,
                downloading,
                queue: s.queue.clone(),
                in_progress: s
                    .items
                    .iter()
                    .filter(|i| !i.played && i.position.is_some_and(|p| p > 0.0))
                    .count() as u32,
            }
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemFilter {
    /// A folder (with its subfolders) or a feed; neither: everything.
    pub folder: Option<String>,
    pub feed: Option<String>,
    /// "all", "unread", "downloaded", "library"; podcasts also
    /// "inProgress" and "unplayed".
    pub show: String,
    /// Items with this topic (arXiv: "cs.AI").
    pub topic: Option<String>,
    pub search: Option<String>,
    /// Items still listed under "unread" (and "unplayed") though they were
    /// just opened, so they do not vanish while being read.
    #[serde(default)]
    pub keep: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TopicDto {
    pub code: String,
    /// arXiv's name for it, when it is an arXiv category.
    pub name: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ItemsDto {
    pub items: Vec<FeedItem>,
    /// How many match, when more than are listed.
    pub total: u32,
    /// Topics of the items shown by folder or feed (before the topic and
    /// search filters), most common first.
    pub topics: Vec<TopicDto>,
}

const MAX_LISTED: usize = 600;

/// The items that match, newest first.
#[tauri::command]
#[specta::specta]
pub async fn feed_items(app: AppHandle, space: Space, filter: ItemFilter) -> AppResult<ItemsDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        read(&state, space, |s| {
            let feeds: Option<HashSet<String>> = match (&filter.feed, &filter.folder) {
                (Some(f), _) => Some(HashSet::from([f.clone()])),
                (None, Some(folder)) => {
                    let inside = s.folder_and_inside(folder);
                    Some(
                        s.feeds
                            .iter()
                            .filter(|f| f.folder.as_ref().is_some_and(|x| inside.contains(x)))
                            .map(|f| f.id.clone())
                            .collect(),
                    )
                }
                _ => None,
            };
            let placed: Vec<&FeedItem> = s
                .items
                .iter()
                .filter(|i| feeds.as_ref().is_none_or(|f| f.contains(&i.feed)))
                .filter(|i| match filter.show.as_str() {
                    "unread" => !i.read || filter.keep.contains(&i.id),
                    "downloaded" => i.file.is_some(),
                    "library" => i.book.is_some(),
                    "inProgress" => i.position.is_some_and(|p| p > 0.0) && !i.played,
                    "unplayed" => !i.played || filter.keep.contains(&i.id),
                    _ => true,
                })
                .collect();
            let mut counts: std::collections::HashMap<&str, u32> = Default::default();
            for i in &placed {
                for t in &i.entry.topics {
                    *counts.entry(t.as_str()).or_default() += 1;
                }
            }
            let mut topics: Vec<TopicDto> = counts
                .into_iter()
                .map(|(code, count)| TopicDto {
                    code: code.to_owned(),
                    name: libreri_feeds::sources::arxiv_category(code).map(|(_, n)| n.to_owned()),
                    count,
                })
                .collect();
            topics.sort_by(|a, b| b.count.cmp(&a.count).then(a.code.cmp(&b.code)));
            topics.truncate(40);
            let words: Vec<String> = filter
                .search
                .as_deref()
                .unwrap_or("")
                .split_whitespace()
                .map(str::to_lowercase)
                .collect();
            let mut items: Vec<FeedItem> = placed
                .into_iter()
                .filter(|i| {
                    filter
                        .topic
                        .as_ref()
                        .is_none_or(|t| i.entry.topics.contains(t))
                })
                .filter(|i| {
                    if words.is_empty() {
                        return true;
                    }
                    let hay = format!(
                        "{} {} {} {}",
                        i.entry.title,
                        i.entry.authors.join(" "),
                        i.entry.summary,
                        i.source
                    )
                    .to_lowercase();
                    words.iter().all(|w| hay.contains(w))
                })
                .cloned()
                .collect();
            let when = |i: &FeedItem| {
                i.entry
                    .published
                    .clone()
                    .unwrap_or_else(|| i.found_at.clone())
            };
            items.sort_by(|a, b| when(b).cmp(&when(a)).then(b.found_at.cmp(&a.found_at)));
            let total = items.len() as u32;
            items.truncate(MAX_LISTED);
            ItemsDto {
                items,
                total,
                topics,
            }
        })
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedPreviewDto {
    /// The feed's own address (found from a site's address if need be).
    pub url: String,
    pub title: String,
    pub site: Option<String>,
    pub count: u32,
    /// The newest few titles.
    pub sample: Vec<String>,
    /// Already followed, under this title.
    pub followed: Option<String>,
}

/// Finds the feed for an address (a feed's, or a site's).
#[tauri::command]
#[specta::specta]
pub async fn feed_find(app: AppHandle, space: Space, address: String) -> AppResult<FeedPreviewDto> {
    blocking(move || {
        let found = libreri_feeds::fetch::discover(&address).map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let followed = read(&state, space, |s| {
            s.feeds
                .iter()
                .find(|f| f.url == found.url)
                .map(|f| f.title.clone())
        })?;
        Ok(FeedPreviewDto {
            url: found.url,
            title: found.parsed.title,
            site: found.parsed.site,
            count: found.parsed.entries.len() as u32,
            sample: found
                .parsed
                .entries
                .iter()
                .take(3)
                .map(|e| e.title.clone())
                .collect(),
            followed,
        })
    })
    .await
}

/// Follows a feed. Its items are fetched by [`feeds_refresh`].
#[tauri::command]
#[specta::specta]
pub async fn feed_add(
    app: AppHandle,
    space: Space,
    url: String,
    title: String,
    folder: Option<String>,
    auto_download: bool,
) -> AppResult<String> {
    blocking(move || {
        let url = libreri_feeds::fetch::normalise(&url).map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let id = change(&state, space, |_, s| {
            let id = s
                .add_feed(&url, &title, None, folder.as_deref())
                .map_err(AppError::invalid)?;
            s.change_feed(&id, None, None, Some(auto_download))
                .map_err(AppError::invalid)?;
            Ok(id)
        })?;
        changed(&app, 0);
        Ok(id)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub fn arxiv_categories() -> Vec<ArxivGroup> {
    libreri_feeds::sources::arxiv_groups()
}

#[tauri::command]
#[specta::specta]
pub fn suggested_feeds() -> Vec<Suggested> {
    libreri_feeds::sources::suggested()
}

/// Follows arXiv categories (each in "arXiv › <group>") and, optionally,
/// a search. Returns the new feeds' ids.
#[tauri::command]
#[specta::specta]
pub async fn feeds_add_arxiv(
    app: AppHandle,
    codes: Vec<String>,
    search: Option<String>,
    auto_download: bool,
) -> AppResult<Vec<String>> {
    let space = Space::Feeds;
    blocking(move || {
        let search_url = search
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(|q| libreri_feeds::sources::arxiv_search_url(q).map(|u| (q.to_owned(), u)))
            .transpose()
            .map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let ids = change(&state, space, |_, s| {
            let mut ids = Vec::new();
            for code in &codes {
                let Some((group, name)) = libreri_feeds::sources::arxiv_category(code) else {
                    return Err(AppError::invalid(format!(
                        "“{code}” is not an arXiv category"
                    )));
                };
                let url = libreri_feeds::sources::arxiv_feed_url(code);
                if s.feeds.iter().any(|f| f.url == url) {
                    continue;
                }
                let folder = s
                    .add_folder_path(&["arXiv", group], None)
                    .map_err(AppError::invalid)?;
                let id = s
                    .add_feed(
                        &url,
                        &format!("{name} ({code})"),
                        Some("https://arxiv.org".into()),
                        folder.as_deref(),
                    )
                    .map_err(AppError::invalid)?;
                s.change_feed(&id, None, None, Some(auto_download))
                    .map_err(AppError::invalid)?;
                ids.push(id);
            }
            if let Some((q, url)) = &search_url {
                if !s.feeds.iter().any(|f| &f.url == url) {
                    let folder = s
                        .add_folder_path(&["arXiv", "Searches"], None)
                        .map_err(AppError::invalid)?;
                    let id = s
                        .add_feed(
                            url,
                            &format!("arXiv: {q}"),
                            Some("https://arxiv.org".into()),
                            folder.as_deref(),
                        )
                        .map_err(AppError::invalid)?;
                    s.change_feed(&id, None, None, Some(auto_download))
                        .map_err(AppError::invalid)?;
                    ids.push(id);
                }
            }
            Ok(ids)
        })?;
        changed(&app, 0);
        Ok(ids)
    })
    .await
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FeedChange {
    pub title: Option<String>,
    /// Move to this folder ("" = the top level).
    pub folder: Option<String>,
    pub auto_download: Option<bool>,
    /// Podcasts: play at this speed (0: the usual speed).
    pub speed: Option<f64>,
}

fn place(folder: &Option<String>) -> Option<Option<&str>> {
    folder
        .as_deref()
        .map(|f| if f.is_empty() { None } else { Some(f) })
}

#[tauri::command]
#[specta::specta]
pub async fn feed_change(
    app: AppHandle,
    space: Space,
    id: String,
    change_to: FeedChange,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            s.change_feed(
                &id,
                change_to.title.as_deref(),
                place(&change_to.folder),
                change_to.auto_download,
            )
            .map_err(AppError::invalid)?;
            if let Some(v) = change_to.speed {
                if let Some(f) = s.feeds.iter_mut().find(|f| f.id == id) {
                    f.speed = (v > 0.0).then_some(v.clamp(0.5, 3.0));
                }
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Stops following a feed. Downloads stay.
#[tauri::command]
#[specta::specta]
pub async fn feed_remove(app: AppHandle, space: Space, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            s.remove_feed(&id);
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn feed_folder_add(
    app: AppHandle,
    space: Space,
    name: String,
    parent: Option<String>,
) -> AppResult<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let id = change(&state, space, |_, s| {
            s.add_folder(&name, parent.as_deref())
                .map_err(AppError::invalid)
        })?;
        changed(&app, 0);
        Ok(id)
    })
    .await
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderChange {
    pub name: Option<String>,
    /// Move into this folder ("" = the top level).
    pub parent: Option<String>,
    pub auto_download: Option<bool>,
}

#[tauri::command]
#[specta::specta]
pub async fn feed_folder_change(
    app: AppHandle,
    space: Space,
    id: String,
    change_to: FolderChange,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            s.change_folder(
                &id,
                change_to.name.as_deref(),
                place(&change_to.parent),
                change_to.auto_download,
            )
            .map_err(AppError::invalid)
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Removes a folder; its feeds and folders move up a level.
#[tauri::command]
#[specta::specta]
pub async fn feed_folder_remove(app: AppHandle, space: Space, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            s.remove_folder(&id).map_err(AppError::invalid)
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn feeds_settings_set(
    app: AppHandle,
    space: Space,
    settings: FeedSettings,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            s.settings = FeedSettings {
                refresh_minutes: settings.refresh_minutes.min(24 * 60),
                keep_days: settings.keep_days.clamp(1, 3650),
            };
            s.prune(chrono::Utc::now());
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

#[derive(Debug, Clone, Default, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RefreshReport {
    pub new_items: u32,
    pub failed: u32,
    pub downloaded: u32,
    /// Another check was already running.
    pub busy: bool,
}

/// Feeds read at the same time.
const AT_ONCE: usize = 6;

/// Looks for new items in every feed (or those given), then downloads the
/// new items of feeds set to download by themselves.
#[tauri::command]
#[specta::specta]
pub async fn feeds_refresh(
    app: AppHandle,
    space: Space,
    ids: Option<Vec<String>>,
) -> AppResult<RefreshReport> {
    blocking(move || {
        let state = app.state::<AppState>();
        if space.refreshing(&state).swap(true, Ordering::SeqCst) {
            return Ok(RefreshReport {
                busy: true,
                ..Default::default()
            });
        }
        changed(&app, 0);
        let result = refresh(&app, space, ids);
        space.refreshing(&state).store(false, Ordering::SeqCst);
        changed(&app, result.as_ref().map_or(0, |r| r.new_items));
        result
    })
    .await
}

type Fetched = (String, Result<Option<libreri_feeds::fetch::Update>, String>);

fn refresh(app: &AppHandle, space: Space, ids: Option<Vec<String>>) -> AppResult<RefreshReport> {
    let state = app.state::<AppState>();
    let lib = state.library()?;
    let profile = lib.profile()?;
    let now = chrono::Utc::now();
    let wanted = change(&state, space, |_, s| {
        s.prune(now);
        Ok(s.feeds
            .iter()
            .filter(|f| ids.as_ref().is_none_or(|ids| ids.contains(&f.id)))
            .map(|f| {
                (
                    f.id.clone(),
                    f.url.clone(),
                    f.etag.clone(),
                    f.last_modified.clone(),
                )
            })
            .collect::<Vec<_>>())
    })?;
    let mut results: Vec<Fetched> = Vec::new();
    for chunk in wanted.chunks(AT_ONCE) {
        let got: Vec<Fetched> = std::thread::scope(|scope| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|(id, url, etag, lm)| {
                    scope.spawn(move || {
                        (
                            id.clone(),
                            libreri_feeds::fetch::read(url, etag.as_deref(), lm.as_deref()),
                        )
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        results.extend(got);
    }
    // Someone else signed in meanwhile: their feeds are not these.
    if state.library()?.profile().ok().as_ref() != Some(&profile) {
        return Ok(RefreshReport::default());
    }
    let mut report = RefreshReport::default();
    let checked = chrono::Utc::now().to_rfc3339();
    let fresh: Vec<String> = change(&state, space, |_, s| {
        let mut fresh = Vec::new();
        for (id, result) in &results {
            match result {
                Ok(Some(update)) => {
                    let added = s.merge(id, &update.parsed, now);
                    if let Some(f) = s.feeds.iter_mut().find(|f| &f.id == id) {
                        f.etag = update.etag.clone();
                        f.last_modified = update.last_modified.clone();
                        f.checked_at = Some(checked.clone());
                        f.error = None;
                    }
                    if s.auto_download(id) {
                        fresh.extend(added.iter().cloned());
                    }
                    report.new_items += added.len() as u32;
                }
                Ok(None) => {
                    if let Some(f) = s.feeds.iter_mut().find(|f| &f.id == id) {
                        f.checked_at = Some(checked.clone());
                        f.error = None;
                    }
                }
                Err(e) => {
                    report.failed += 1;
                    if let Some(f) = s.feeds.iter_mut().find(|f| &f.id == id) {
                        f.checked_at = Some(checked.clone());
                        f.error = Some(e.clone());
                    }
                }
            }
        }
        Ok(fresh)
    })?;
    changed(app, report.new_items);
    if space == Space::Podcasts {
        artworks(app, space);
    }
    for id in fresh {
        if state.library()?.profile().ok().as_ref() != Some(&profile) {
            break;
        }
        if download(app, space, &id).is_ok() {
            report.downloaded += 1;
        }
        changed(app, 0);
    }
    Ok(report)
}

/// Removes an item from the "downloading" list when dropped.
struct Downloading<'a>(&'a AppState, String);

impl Drop for Downloading<'_> {
    fn drop(&mut self) {
        self.0
            .feed_downloads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.1);
    }
}

/// Downloads an item: its PDF, or a readable copy of its page as
/// Markdown. Returns the item.
fn download(app: &AppHandle, space: Space, id: &str) -> AppResult<FeedItem> {
    let state = app.state::<AppState>();
    if !state
        .feed_downloads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(space.tag(id))
    {
        return Err(AppError::invalid("this item is already being downloaded"));
    }
    let _mark = Downloading(&state, space.tag(id));
    changed(app, 0);
    let (item, folders) = read(&state, space, |s| {
        s.item(id).cloned().map(|item| {
            let mut folders = s.feed(&item.feed).map_or_else(Vec::new, |f| {
                let mut p = s.folder_path(f.folder.as_deref());
                p.push(f.title.clone());
                p
            });
            if folders.is_empty() {
                folders.push(item.source.clone());
            }
            if let Some(top) = space.downloads() {
                folders.insert(0, top.to_owned());
            }
            (item, folders)
        })
    })?
    .ok_or_else(|| AppError::invalid("that item is no longer there"))?;
    if item.file.is_some() {
        return Ok(item);
    }
    let lib = state.library()?;
    let saved = (|| -> AppResult<String> {
        // Podcast episodes: the audio, written straight to disk.
        if let Some(audio) = item.entry.audio.as_deref() {
            let ext =
                libreri_feeds::podcasts::audio_extension(audio, item.entry.audio_type.as_deref());
            let (path, rel) = lib.new_feed_file(&folders, &item.entry.title, ext)?;
            libreri_feeds::podcasts::download_audio(audio, &path).map_err(AppError::invalid)?;
            return Ok(rel);
        }
        let got =
            libreri_feeds::fetch::download(item.entry.pdf.as_deref(), item.entry.link.as_deref())
                .map_err(AppError::invalid)?;
        match got {
            Download::Pdf(bytes) => {
                Ok(lib.save_feed_file(&folders, &item.entry.title, "pdf", &bytes)?)
            }
            Download::Page { html, url } => {
                let article =
                    libreri_links::copy::readable(&html, &url, libreri_links::copy::web_pictures())
                        .map_err(AppError::invalid)?;
                let when = chrono::Local::now().format("%-d %B %Y").to_string();
                let md = libreri_feeds::article::markdown(&item, &article, &url, &when);
                Ok(lib.save_feed_file(&folders, &item.entry.title, "md", md.as_bytes())?)
            }
        }
    })();
    let out = change(&state, space, |lib, s| {
        let Some(it) = s.item_mut(id) else {
            // Deleted while downloading: the file goes too.
            if let Ok(rel) = &saved {
                let _ = lib.delete_feed_file(rel);
            }
            return Err(AppError::invalid("that item was deleted"));
        };
        match &saved {
            Ok(rel) => {
                it.file = Some(rel.clone());
                it.download_error = None;
            }
            Err(e) => it.download_error = Some(e.message.clone()),
        }
        Ok(it.clone())
    });
    changed(app, 0);
    saved?;
    out
}

#[tauri::command]
#[specta::specta]
pub async fn feed_item_download(app: AppHandle, space: Space, id: String) -> AppResult<FeedItem> {
    blocking(move || download(&app, space, &id)).await
}

/// Deletes items (and their downloads). They do not come back.
#[tauri::command]
#[specta::specta]
pub async fn feed_items_delete(app: AppHandle, space: Space, ids: Vec<String>) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |lib, s| {
            let now = chrono::Utc::now();
            for id in &ids {
                if let Some(file) = s.delete_item(id, now) {
                    lib.delete_feed_file(&file)?;
                }
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Deletes an item's download only (the item stays, to download again).
#[tauri::command]
#[specta::specta]
pub async fn feed_item_forget_file(app: AppHandle, space: Space, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |lib, s| {
            if let Some(it) = s.item_mut(&id) {
                if let Some(file) = it.file.take() {
                    lib.delete_feed_file(&file)?;
                }
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn feed_items_read(
    app: AppHandle,
    space: Space,
    ids: Vec<String>,
    read_now: bool,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            for id in &ids {
                if let Some(it) = s.item_mut(id) {
                    it.read = read_now;
                }
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Marks everything shown by a folder or feed (or all) as read.
#[tauri::command]
#[specta::specta]
pub async fn feed_all_read(
    app: AppHandle,
    space: Space,
    folder: Option<String>,
    feed: Option<String>,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, space, |_, s| {
            let feeds: Option<HashSet<String>> = match (&feed, &folder) {
                (Some(f), _) => Some(HashSet::from([f.clone()])),
                (None, Some(folder)) => {
                    let inside = s.folder_and_inside(folder);
                    Some(
                        s.feeds
                            .iter()
                            .filter(|f| f.folder.as_ref().is_some_and(|x| inside.contains(x)))
                            .map(|f| f.id.clone())
                            .collect(),
                    )
                }
                _ => None,
            };
            for it in &mut s.items {
                if feeds.as_ref().is_none_or(|f| f.contains(&it.feed)) {
                    it.read = true;
                }
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// The details a feed gives an item, for the book it becomes.
fn details_of(item: &FeedItem, file: &str) -> BookMetadata {
    let e = &item.entry;
    let preprint = e.arxiv_id.is_some()
        || ["arxiv", "biorxiv", "medrxiv", "ssrn"]
            .iter()
            .any(|p| item.source.to_lowercase().contains(p));
    let audio = e.audio.is_some();
    let content_type = if audio {
        ContentType::Audiobook
    } else if file.ends_with(".md") {
        ContentType::Article
    } else if preprint {
        ContentType::Preprint
    } else if e.doi.is_some() {
        ContentType::ResearchPaper
    } else {
        ContentType::Article
    };
    let tags = e
        .topics
        .iter()
        .map(|t| {
            libreri_feeds::sources::arxiv_category(t)
                .map_or_else(|| t.clone(), |(_, name)| name.to_owned())
        })
        .collect();
    // arXiv categories also give the category tree: "Computer Science/Machine Learning".
    let mut categories: Vec<String> = Vec::new();
    for t in &e.topics {
        if let Some((group, name)) = libreri_feeds::sources::arxiv_category(t) {
            let c = format!("{group}/{name}");
            if !categories.contains(&c) {
                categories.push(c);
            }
        }
    }
    let source = Some(item.source.clone()).filter(|s| !s.is_empty());
    // A journal's feed (a paper with a DOI, not a preprint): the feed is
    // the journal.
    let journal = (content_type == ContentType::ResearchPaper)
        .then(|| source.clone())
        .flatten();
    let publisher = if e.arxiv_id.is_some() {
        Some("arXiv".to_owned())
    } else if journal.is_some() {
        None
    } else {
        source
    };
    BookMetadata {
        title: e.title.clone(),
        authors: e.authors.clone(),
        about: Some(e.summary.clone()).filter(|s| !s.is_empty()),
        year: e
            .published
            .as_deref()
            .and_then(|d| d.get(..4))
            .and_then(|y| y.parse().ok()),
        publisher,
        doi: e.doi.clone(),
        arxiv_id: e.arxiv_id.clone(),
        url: e.link.clone(),
        tags,
        categories,
        journal,
        content_type,
        // A podcast episode: the show is the series.
        series: audio.then(|| item.source.clone()).filter(|s| !s.is_empty()),
        ..Default::default()
    }
}

/// Adds an item to the library, in `folder` (relative to `Books/`),
/// downloading it first if need be. Returns the book's id.
#[tauri::command]
#[specta::specta]
pub async fn feed_item_to_library(
    app: AppHandle,
    space: Space,
    id: String,
    folder: String,
) -> AppResult<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.library()?.require_edit()?;
        let item = download(&app, space, &id)?;
        let file = item
            .file
            .clone()
            .ok_or_else(|| AppError::invalid("the item could not be downloaded"))?;
        let lib = state.library()?;
        let book = lib.add_feed_file_to_library(&file, &folder, &details_of(&item, &file))?;
        change(&state, space, |_, s| {
            if let Some(it) = s.item_mut(&id) {
                it.file = None;
                it.book = Some(book.to_string());
                it.read = true;
            }
            Ok(())
        })?;
        changed(&app, 0);
        let _ = crate::events::LibraryChanged::default().emit(&app);
        // What the feed did not say (journal volume and issue, pages,
        // language…) from the online sources, as for any import.
        let _ = state.fill_after_feed(book.clone());
        Ok(book.to_string())
    })
    .await
}

/// Adds the feeds of an OPML file (in `parent`, or at the top level).
#[tauri::command]
#[specta::specta]
pub async fn feeds_import_opml(
    app: AppHandle,
    space: Space,
    path: String,
    parent: Option<String>,
) -> AppResult<(u32, u32)> {
    blocking(move || {
        let xml = std::fs::read_to_string(&path)
            .map_err(|_| AppError::invalid("the OPML file could not be read"))?;
        let nodes = libreri_feeds::opml::read(&xml).map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let counts = change(&state, space, |_, s| {
            s.add_opml(&nodes, parent.as_deref())
                .map_err(AppError::invalid)
        })?;
        changed(&app, 0);
        Ok(counts)
    })
    .await
}

/// Writes the folders and feeds to an OPML file.
#[tauri::command]
#[specta::specta]
pub async fn feeds_export_opml(app: AppHandle, space: Space, path: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        let nodes = read(&state, space, |s| s.opml())?;
        let xml = libreri_feeds::opml::write("Libreri feeds", &nodes);
        std::fs::write(&path, xml).map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?;
        Ok(())
    })
    .await
}

/// Shows the feeds folder, or a download, in the file manager.
#[tauri::command]
#[specta::specta]
pub async fn feeds_reveal(app: AppHandle, file: Option<String>) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let state = app.state::<AppState>();
    let lib: Arc<Library> = state.library()?;
    let io = |e: tauri_plugin_opener::Error| AppError::new(AppErrorKind::Io, e.to_string());
    match file {
        Some(rel) => {
            let path = lib.own_feed_file(&rel)?;
            app.opener().reveal_item_in_dir(path).map_err(io)
        }
        None => {
            let dir = lib.feeds_folder()?;
            app.opener()
                .open_path(dir.to_string_lossy(), None::<&str>)
                .map_err(io)
        }
    }
}

/// Opens a download in the app the system uses for it.
#[tauri::command]
#[specta::specta]
pub async fn feed_open_file(app: AppHandle, file: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let state = app.state::<AppState>();
    let path = state.library()?.own_feed_file(&file)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))
}

/// Makes a small copy of each show's artwork that has none yet, so it
/// shows offline and nothing is fetched from the shows' sites on display.
fn artworks(app: &AppHandle, space: Space) {
    let state = app.state::<AppState>();
    let Ok(wanted) = read(&state, space, |s| {
        s.feeds
            .iter()
            .filter(|f| f.artwork.is_none())
            .filter_map(|f| f.image.clone().map(|i| (f.id.clone(), i)))
            .collect::<Vec<_>>()
    }) else {
        return;
    };
    for (id, url) in wanted {
        let Ok(bytes) = libreri_feeds::podcasts::fetch_image(&url) else {
            continue;
        };
        let Ok((pic, kind, _, _)) = libreri_thumbs::picture(&bytes, 240) else {
            continue;
        };
        use base64::Engine;
        let data = format!(
            "data:{kind};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(pic)
        );
        let _ = change(&state, space, |_, s| {
            if let Some(f) = s.feeds.iter_mut().find(|f| f.id == id) {
                f.artwork = Some(data);
            }
            Ok(())
        });
        changed(app, 0);
    }
}

// ---- Podcasts -------------------------------------------------------------

fn index_key(state: &AppState) -> Option<libreri_feeds::podcasts::IndexKey> {
    let s = state.online_settings();
    let key = s.podcastindex_key.filter(|k| !k.trim().is_empty())?;
    let secret = s.podcastindex_secret.filter(|k| !k.trim().is_empty())?;
    Some(libreri_feeds::podcasts::IndexKey { key, secret })
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PodcastIndexDto {
    /// A key and secret are saved on this computer.
    pub configured: bool,
    /// The key's last characters, to recognise it.
    pub key_hint: Option<String>,
}

fn index_dto(state: &AppState) -> PodcastIndexDto {
    let s = state.online_settings();
    let key = s.podcastindex_key.filter(|k| !k.trim().is_empty());
    PodcastIndexDto {
        configured: key.is_some() && s.podcastindex_secret.is_some_and(|k| !k.trim().is_empty()),
        key_hint: key.map(|k| {
            let tail: String = k
                .trim()
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("…{tail}")
        }),
    }
}

/// Whether a Podcast Index key is set on this computer.
#[tauri::command]
#[specta::specta]
pub fn podcast_index_status(app: AppHandle) -> PodcastIndexDto {
    index_dto(&app.state::<AppState>())
}

/// Saves (after checking it) or removes (empty key) a Podcast Index key
/// and secret. Kept on this computer only, never exported.
#[tauri::command]
#[specta::specta]
pub async fn podcast_index_set(
    app: AppHandle,
    key: String,
    secret: String,
) -> AppResult<PodcastIndexDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let key = key.trim().to_owned();
        let secret = secret.trim().to_owned();
        if key.is_empty() {
            state
                .update_online(|s| {
                    s.podcastindex_key = None;
                    s.podcastindex_secret = None;
                })
                .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?;
            return Ok(index_dto(&state));
        }
        if secret.is_empty() {
            return Err(AppError::invalid("enter the secret too"));
        }
        libreri_feeds::podcasts::check_key(&libreri_feeds::podcasts::IndexKey {
            key: key.clone(),
            secret: secret.clone(),
        })
        .map_err(AppError::invalid)?;
        state
            .update_online(|s| {
                s.podcastindex_key = Some(key);
                s.podcastindex_secret = Some(secret);
            })
            .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?;
        Ok(index_dto(&state))
    })
    .await
}

/// Shows matching the words: Apple's podcast search, or Podcast Index
/// (`index`) when a key is set.
#[tauri::command]
#[specta::specta]
pub async fn podcast_search(
    app: AppHandle,
    query: String,
    index: bool,
) -> AppResult<Vec<libreri_feeds::podcasts::Show>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let result = if index {
            let key = index_key(&state).ok_or_else(|| {
                AppError::invalid("add your Podcast Index key in Settings › Online details")
            })?;
            libreri_feeds::podcasts::search_index(&key, &query)
        } else {
            libreri_feeds::podcasts::search_apple(&query)
        };
        result.map_err(AppError::invalid)
    })
    .await
}

/// Popular shows on Podcast Index (needs a key), optionally in a category.
#[tauri::command]
#[specta::specta]
pub async fn podcast_trending(
    app: AppHandle,
    category: Option<String>,
) -> AppResult<Vec<libreri_feeds::podcasts::Show>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let key = index_key(&state).ok_or_else(|| {
            AppError::invalid("add your Podcast Index key in Settings › Online details")
        })?;
        libreri_feeds::podcasts::trending(&key, category.as_deref(), None)
            .map_err(AppError::invalid)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn podcast_categories(app: AppHandle) -> AppResult<Vec<String>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let key = index_key(&state).ok_or_else(|| {
            AppError::invalid("add your Podcast Index key in Settings › Online details")
        })?;
        libreri_feeds::podcasts::categories(&key).map_err(AppError::invalid)
    })
    .await
}

/// Keeps where listening stopped, and whether the episode was finished.
#[tauri::command]
#[specta::specta]
pub async fn podcast_progress(
    app: AppHandle,
    id: String,
    position: f64,
    duration: Option<f64>,
    played: bool,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, Space::Podcasts, |_, s| {
            if let Some(it) = s.item_mut(&id) {
                it.position = Some(position.max(0.0));
                it.read = true;
                if let Some(d) = duration.filter(|d| d.is_finite() && *d > 0.0) {
                    it.entry.duration = Some(d);
                }
                if played {
                    it.played = true;
                    it.position = None;
                }
            }
            if played {
                s.queue.retain(|q| q != &id);
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Marks episodes played (or not), keeping no position.
#[tauri::command]
#[specta::specta]
pub async fn podcast_played(app: AppHandle, ids: Vec<String>, played: bool) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, Space::Podcasts, |_, s| {
            for id in &ids {
                if let Some(it) = s.item_mut(id) {
                    it.played = played;
                    it.read = true;
                    it.position = None;
                }
            }
            if played {
                s.queue.retain(|q| !ids.contains(q));
            }
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Sets the Up next queue (episode ids, in order).
#[tauri::command]
#[specta::specta]
pub async fn podcast_queue_set(app: AppHandle, ids: Vec<String>) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, Space::Podcasts, |_, s| {
            let mut seen = HashSet::new();
            s.queue = ids
                .into_iter()
                .filter(|id| s.items.iter().any(|i| &i.id == id) && seen.insert(id.clone()))
                .collect();
            Ok(())
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Episodes by id (for the queue and the player).
#[tauri::command]
#[specta::specta]
pub async fn podcast_episodes(app: AppHandle, ids: Vec<String>) -> AppResult<Vec<FeedItem>> {
    blocking(move || {
        let state = app.state::<AppState>();
        read(&state, Space::Podcasts, |s| {
            ids.iter().filter_map(|id| s.item(id).cloned()).collect()
        })
    })
    .await
}

/// An episode's transcript: the show's own, or one written down on this
/// computer; and whether one can be written (downloaded, a speech model).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDto {
    pub cues: Vec<libreri_feeds::podcasts::Cue>,
    /// "show", "written" (on this computer, complete), "partial" (being
    /// written, or stopped part way) or "none".
    pub source: String,
    pub downloaded: bool,
    pub has_model: bool,
}

/// Where a written transcript is kept: beside the download, hidden.
fn written_path(audio: &std::path::Path) -> std::path::PathBuf {
    let name = audio
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    audio.with_file_name(format!(".{name}.transcript.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Written {
    complete: bool,
    cues: Vec<libreri_feeds::podcasts::Cue>,
}

fn downloaded_file(state: &AppState, item: &FeedItem) -> Option<std::path::PathBuf> {
    let rel = item.file.as_deref()?;
    let lib = state.library().ok()?;
    lib.own_feed_file(rel).ok().filter(|p| p.is_file())
}

#[tauri::command]
#[specta::specta]
pub async fn podcast_transcript(app: AppHandle, id: String) -> AppResult<TranscriptDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = read(&state, Space::Podcasts, |s| s.item(&id).cloned())?
            .ok_or_else(|| AppError::invalid("that episode is no longer there"))?;
        let file = downloaded_file(&state, &item);
        let has_model = libreri_speech::models(&state.whisper_dir)
            .iter()
            .any(|m| m.downloaded);
        let mut dto = TranscriptDto {
            cues: Vec::new(),
            source: "none".into(),
            downloaded: file.is_some(),
            has_model,
        };
        if let Some(url) = item.entry.transcript.as_deref() {
            match libreri_feeds::podcasts::transcript(url, item.entry.transcript_type.as_deref()) {
                Ok(cues) if !cues.is_empty() => {
                    dto.cues = cues;
                    dto.source = "show".into();
                    return Ok(dto);
                }
                Ok(_) => {}
                // Offline: a written one may still be there.
                Err(e) if file.is_none() => return Err(AppError::invalid(e)),
                Err(_) => {}
            }
        }
        if let Some(w) = file
            .as_deref()
            .and_then(|f| std::fs::read_to_string(written_path(f)).ok())
            .and_then(|s| serde_json::from_str::<Written>(&s).ok())
        {
            dto.source = if w.complete { "written" } else { "partial" }.into();
            dto.cues = w.cues;
        }
        Ok(dto)
    })
    .await
}

/// Minutes of audio written down at a time (saved after each).
const PIECE_SECONDS: f64 = 300.0;

/// Writes an episode's transcript on this computer with the speech model,
/// piece by piece (the panel shows each piece as it is done). The episode
/// must be downloaded. Returns the job id.
#[tauri::command]
#[specta::specta]
pub fn podcast_write_transcript(app: AppHandle, id: String) -> AppResult<String> {
    let state = app.state::<AppState>();
    let item = read(&state, Space::Podcasts, |s| s.item(&id).cloned())?
        .ok_or_else(|| AppError::invalid("that episode is no longer there"))?;
    let file = downloaded_file(&state, &item)
        .ok_or_else(|| AppError::invalid("download the episode first"))?;
    if !libreri_speech::models(&state.whisper_dir)
        .iter()
        .any(|m| m.downloaded)
    {
        return Err(AppError::new(
            AppErrorKind::NotFound,
            "download a speech model first (Settings › Speech)",
        ));
    }
    let language = state
        .settings
        .lock()
        .map(|s| s.speech.language.clone())
        .unwrap_or(None);
    let handle = app.clone();
    let label = format!("Writing down “{}”", item.entry.title);
    let job = state.jobs.submit(label, move |ctx| {
        use libreri_jobs::JobError;
        let failed = |e: String| JobError::Failed(e);
        let transcriber = handle
            .state::<AppState>()
            .transcriber()
            .map_err(|e| failed(e.to_string()))?;
        let total = libreri_speech::audio::duration(&file)
            .ok_or_else(|| failed("the episode's length is unknown".into()))?;
        let out = written_path(&file);
        let save = |w: &Written| {
            let json = serde_json::to_string(w).map_err(|e| failed(e.to_string()))?;
            std::fs::write(&out, json).map_err(|e| failed(e.to_string()))
        };
        let mut written = Written {
            complete: false,
            cues: Vec::new(),
        };
        let mut at = 0.0;
        while at < total {
            ctx.check_cancelled()?;
            ctx.progress(
                at as u64,
                total as u64,
                Some(format!(
                    "{} of {} min",
                    (at / 60.0) as u64,
                    (total / 60.0).ceil() as u64
                )),
            );
            let (samples, _) =
                libreri_speech::audio::decode(&file, at, Some(PIECE_SECONDS)).map_err(failed)?;
            let segs = transcriber
                .transcribe(&samples, language.as_deref(), None)
                .map_err(failed)?;
            written
                .cues
                .extend(segs.into_iter().map(|s| libreri_feeds::podcasts::Cue {
                    start: Some(at + s.start),
                    end: Some(at + s.end),
                    speaker: None,
                    text: s.text,
                }));
            at += PIECE_SECONDS;
            save(&written)?;
        }
        written.complete = true;
        save(&written)?;
        ctx.progress(total as u64, total as u64, None);
        Ok(())
    });
    Ok(job.to_string())
}

/// An episode's chapters, from the show (none when it has none).
#[tauri::command]
#[specta::specta]
pub async fn podcast_chapters(
    app: AppHandle,
    id: String,
) -> AppResult<Vec<libreri_feeds::podcasts::Chapter>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = read(&state, Space::Podcasts, |s| s.item(&id).cloned())?
            .ok_or_else(|| AppError::invalid("that episode is no longer there"))?;
        let Some(url) = item.entry.chapters.as_deref() else {
            return Ok(Vec::new());
        };
        libreri_feeds::podcasts::chapters(url).map_err(AppError::invalid)
    })
    .await
}
