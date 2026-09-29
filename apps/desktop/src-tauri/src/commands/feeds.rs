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

fn load(lib: &Library) -> AppResult<State> {
    match lib.read_feeds()? {
        Some(json) => State::from_json(&json).map_err(AppError::invalid),
        None => Ok(State::default()),
    }
}

fn save(lib: &Library, s: &State) -> AppResult<()> {
    lib.write_feeds(&s.to_json())?;
    Ok(())
}

/// Loads, changes and saves the state, one change at a time.
fn change<T>(
    state: &AppState,
    f: impl FnOnce(&Library, &mut State) -> AppResult<T>,
) -> AppResult<T> {
    let lib = state.library()?;
    let _guard = state
        .feeds_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut s = load(&lib)?;
    let out = f(&lib, &mut s)?;
    save(&lib, &s)?;
    Ok(out)
}

fn read<T>(state: &AppState, f: impl FnOnce(&State) -> T) -> AppResult<T> {
    let lib = state.library()?;
    let _guard = state
        .feeds_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(f(&load(&lib)?))
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
}

/// The folders and feeds, with counts.
#[tauri::command]
#[specta::specta]
pub async fn feeds_overview(app: AppHandle) -> AppResult<FeedsDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let downloading: Vec<String> = state
            .feed_downloads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect();
        let refreshing = state.feeds_refreshing.load(Ordering::SeqCst);
        read(&state, |s| {
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
    /// "all", "unread", "downloaded" or "library".
    pub show: String,
    /// Items with this topic (arXiv: "cs.AI").
    pub topic: Option<String>,
    pub search: Option<String>,
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
pub async fn feed_items(app: AppHandle, filter: ItemFilter) -> AppResult<ItemsDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        read(&state, |s| {
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
                    "unread" => !i.read,
                    "downloaded" => i.file.is_some(),
                    "library" => i.book.is_some(),
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
pub async fn feed_find(app: AppHandle, address: String) -> AppResult<FeedPreviewDto> {
    blocking(move || {
        let found = libreri_feeds::fetch::discover(&address).map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let followed = read(&state, |s| {
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
    url: String,
    title: String,
    folder: Option<String>,
    auto_download: bool,
) -> AppResult<String> {
    blocking(move || {
        let url = libreri_feeds::fetch::normalise(&url).map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let id = change(&state, |_, s| {
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
    blocking(move || {
        let search_url = search
            .as_deref()
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(|q| libreri_feeds::sources::arxiv_search_url(q).map(|u| (q.to_owned(), u)))
            .transpose()
            .map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let ids = change(&state, |_, s| {
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
}

fn place(folder: &Option<String>) -> Option<Option<&str>> {
    folder
        .as_deref()
        .map(|f| if f.is_empty() { None } else { Some(f) })
}

#[tauri::command]
#[specta::specta]
pub async fn feed_change(app: AppHandle, id: String, change_to: FeedChange) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
            s.change_feed(
                &id,
                change_to.title.as_deref(),
                place(&change_to.folder),
                change_to.auto_download,
            )
            .map_err(AppError::invalid)
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

/// Stops following a feed. Downloads stay.
#[tauri::command]
#[specta::specta]
pub async fn feed_remove(app: AppHandle, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
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
    name: String,
    parent: Option<String>,
) -> AppResult<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let id = change(&state, |_, s| {
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
    id: String,
    change_to: FolderChange,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
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
pub async fn feed_folder_remove(app: AppHandle, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
            s.remove_folder(&id).map_err(AppError::invalid)
        })?;
        changed(&app, 0);
        Ok(())
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn feeds_settings_set(app: AppHandle, settings: FeedSettings) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
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
pub async fn feeds_refresh(app: AppHandle, ids: Option<Vec<String>>) -> AppResult<RefreshReport> {
    blocking(move || {
        let state = app.state::<AppState>();
        if state.feeds_refreshing.swap(true, Ordering::SeqCst) {
            return Ok(RefreshReport {
                busy: true,
                ..Default::default()
            });
        }
        changed(&app, 0);
        let result = refresh(&app, ids);
        state.feeds_refreshing.store(false, Ordering::SeqCst);
        changed(&app, result.as_ref().map_or(0, |r| r.new_items));
        result
    })
    .await
}

type Fetched = (String, Result<Option<libreri_feeds::fetch::Update>, String>);

fn refresh(app: &AppHandle, ids: Option<Vec<String>>) -> AppResult<RefreshReport> {
    let state = app.state::<AppState>();
    let lib = state.library()?;
    let profile = lib.profile()?;
    let now = chrono::Utc::now();
    let wanted = change(&state, |_, s| {
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
    let fresh: Vec<String> = change(&state, |_, s| {
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
    for id in fresh {
        if state.library()?.profile().ok().as_ref() != Some(&profile) {
            break;
        }
        if download(app, &id).is_ok() {
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
fn download(app: &AppHandle, id: &str) -> AppResult<FeedItem> {
    let state = app.state::<AppState>();
    if !state
        .feed_downloads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id.to_owned())
    {
        return Err(AppError::invalid("this item is already being downloaded"));
    }
    let _mark = Downloading(&state, id.to_owned());
    changed(app, 0);
    let (item, folders) = read(&state, |s| {
        s.item(id).cloned().map(|item| {
            let mut folders = s.feed(&item.feed).map_or_else(Vec::new, |f| {
                let mut p = s.folder_path(f.folder.as_deref());
                p.push(f.title.clone());
                p
            });
            if folders.is_empty() {
                folders.push(item.source.clone());
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
    let out = change(&state, |lib, s| {
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
pub async fn feed_item_download(app: AppHandle, id: String) -> AppResult<FeedItem> {
    blocking(move || download(&app, &id)).await
}

/// Deletes items (and their downloads). They do not come back.
#[tauri::command]
#[specta::specta]
pub async fn feed_items_delete(app: AppHandle, ids: Vec<String>) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |lib, s| {
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
pub async fn feed_item_forget_file(app: AppHandle, id: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |lib, s| {
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
pub async fn feed_items_read(app: AppHandle, ids: Vec<String>, read_now: bool) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
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
    folder: Option<String>,
    feed: Option<String>,
) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        change(&state, |_, s| {
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
    let content_type = if file.ends_with(".md") {
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
    let publisher = if e.arxiv_id.is_some() {
        Some("arXiv".to_owned())
    } else {
        Some(item.source.clone()).filter(|s| !s.is_empty())
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
        content_type,
        ..Default::default()
    }
}

/// Adds an item to the library, in `folder` (relative to `Books/`),
/// downloading it first if need be. Returns the book's id.
#[tauri::command]
#[specta::specta]
pub async fn feed_item_to_library(app: AppHandle, id: String, folder: String) -> AppResult<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.library()?.require_edit()?;
        let item = download(&app, &id)?;
        let file = item
            .file
            .clone()
            .ok_or_else(|| AppError::invalid("the item could not be downloaded"))?;
        let lib = state.library()?;
        let book = lib.add_feed_file_to_library(&file, &folder, &details_of(&item, &file))?;
        change(&state, |_, s| {
            if let Some(it) = s.item_mut(&id) {
                it.file = None;
                it.book = Some(book.to_string());
                it.read = true;
            }
            Ok(())
        })?;
        changed(&app, 0);
        let _ = crate::events::LibraryChanged::default().emit(&app);
        Ok(book.to_string())
    })
    .await
}

/// Adds the feeds of an OPML file (in `parent`, or at the top level).
#[tauri::command]
#[specta::specta]
pub async fn feeds_import_opml(
    app: AppHandle,
    path: String,
    parent: Option<String>,
) -> AppResult<(u32, u32)> {
    blocking(move || {
        let xml = std::fs::read_to_string(&path)
            .map_err(|_| AppError::invalid("the OPML file could not be read"))?;
        let nodes = libreri_feeds::opml::read(&xml).map_err(AppError::invalid)?;
        let state = app.state::<AppState>();
        let counts = change(&state, |_, s| {
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
pub async fn feeds_export_opml(app: AppHandle, path: String) -> AppResult<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        let nodes = read(&state, |s| s.opml())?;
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
