//! Profiles: the picker, signing in and out, PINs, managing people, and
//! each profile's preferences and smart collections.

use crate::error::{AppError, AppResult};
use crate::events::SessionChanged;
use crate::state::AppState;
use libreri_core::{BookQuery, Profile, ProfileId, ProfileKind};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

fn announce(app: &AppHandle, profile: Option<&Profile>) {
    let _ = SessionChanged {
        profile_id: profile.map(|p| p.id.to_string()),
    }
    .emit(app);
}

fn profile_id(id: &str) -> AppResult<ProfileId> {
    id.parse()
        .map_err(|_| AppError::invalid("that profile id is not valid"))
}

/// A profile as the picker and Settings show it. Never includes hashes.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub id: String,
    pub name: String,
    pub colour: String,
    pub kind: ProfileKind,
    pub has_pin: bool,
    /// Kids: folders (relative to `Books/`) they may open.
    pub allowed_folders: Vec<String>,
    pub last_used: Option<String>,
    /// Seconds before another PIN may be tried (after wrong ones).
    pub pin_wait: u32,
}

impl ProfileDto {
    fn of(p: &Profile, pin_wait: u64) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            colour: p.colour.clone(),
            kind: p.kind,
            has_pin: p.has_pin(),
            allowed_folders: p.allowed_folders.clone(),
            last_used: p.last_used.clone(),
            pin_wait: pin_wait.min(u64::from(u32::MAX)) as u32,
        }
    }
}

/// Who is signed in and what they may do.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub profile: ProfileDto,
    /// The profile's interface preferences (JSON written by the interface).
    pub prefs: String,
    pub can_edit_library: bool,
    pub can_manage_profiles: bool,
    /// False for guests: nothing they do is kept.
    pub keeps_data: bool,
}

impl SessionDto {
    fn of(p: &Profile) -> Self {
        Self {
            profile: ProfileDto::of(p, 0),
            prefs: p.prefs.clone(),
            can_edit_library: p.kind.can_edit_library(),
            can_manage_profiles: p.kind.can_manage_profiles(),
            keeps_data: p.kind.keeps_data(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn list_profiles(state: State<'_, AppState>) -> AppResult<Vec<ProfileDto>> {
    let library = state.library()?;
    let profiles = library.profiles()?;
    profiles
        .iter()
        .map(|p| Ok(ProfileDto::of(p, library.pin_wait(&p.id)?)))
        .collect()
}

/// The signed-in profile, or nothing when the profile picker should show.
#[tauri::command]
#[specta::specta]
pub fn current_session(state: State<'_, AppState>) -> AppResult<Option<SessionDto>> {
    Ok(state
        .library()?
        .current_profile()?
        .as_ref()
        .map(SessionDto::of))
}

#[tauri::command]
#[specta::specta]
pub fn sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    pin: Option<String>,
) -> AppResult<SessionDto> {
    let p = state
        .library()?
        .sign_in(&profile_id(&id)?, pin.as_deref())?;
    announce(&app, Some(&p));
    Ok(SessionDto::of(&p))
}

/// Locks the library and goes back to the profile picker.
#[tauri::command]
#[specta::specta]
pub fn sign_out(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    state.library()?.sign_out()?;
    announce(&app, None);
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewProfile {
    pub name: String,
    pub colour: String,
    pub kind: ProfileKind,
    pub pin: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub fn create_profile(state: State<'_, AppState>, profile: NewProfile) -> AppResult<ProfileDto> {
    let p = state.library()?.create_profile(
        &profile.name,
        &profile.colour,
        profile.kind,
        profile.pin.as_deref(),
    )?;
    Ok(ProfileDto::of(&p, 0))
}

#[tauri::command]
#[specta::specta]
pub fn update_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
    colour: String,
    kind: ProfileKind,
) -> AppResult<ProfileDto> {
    let p = state
        .library()?
        .update_profile(&profile_id(&id)?, &name, &colour, kind)?;
    Ok(ProfileDto::of(&p, 0))
}

/// Sets, changes or removes a PIN. Returns the owner's new recovery code,
/// which the interface shows once.
#[tauri::command]
#[specta::specta]
pub fn set_profile_pin(
    state: State<'_, AppState>,
    id: String,
    current_pin: Option<String>,
    new_pin: Option<String>,
) -> AppResult<Option<String>> {
    let change = state.library()?.set_pin(
        &profile_id(&id)?,
        current_pin.as_deref(),
        new_pin.as_deref(),
    )?;
    Ok(change.recovery_code)
}

/// Resets the owner's forgotten PIN with the recovery code and signs in.
/// Returns the next recovery code.
#[tauri::command]
#[specta::specta]
pub fn recover_owner(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery_code: String,
    new_pin: String,
) -> AppResult<String> {
    let library = state.library()?;
    let code = library.recover_owner(&recovery_code, &new_pin)?;
    announce(&app, library.current_profile()?.as_ref());
    Ok(code)
}

#[tauri::command]
#[specta::specta]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    Ok(state.library()?.delete_profile(&profile_id(&id)?)?)
}

#[tauri::command]
#[specta::specta]
pub fn set_allowed_folders(
    state: State<'_, AppState>,
    id: String,
    folders: Vec<String>,
) -> AppResult<ProfileDto> {
    let p = state
        .library()?
        .set_allowed_folders(&profile_id(&id)?, folders)?;
    Ok(ProfileDto::of(&p, 0))
}

/// Saves the signed-in profile's interface preferences (JSON).
#[tauri::command]
#[specta::specta]
pub fn save_prefs(state: State<'_, AppState>, prefs: String) -> AppResult<()> {
    Ok(state.library()?.set_prefs(&prefs)?)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    pub id: String,
    pub name: String,
    pub query: BookQuery,
}

impl From<libreri_library::Collection> for CollectionDto {
    fn from(c: libreri_library::Collection) -> Self {
        Self {
            id: c.id,
            name: c.name,
            query: c.query,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<CollectionDto>> {
    Ok(state
        .library()?
        .collections()?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Adds (empty `id`) or updates a smart collection.
#[tauri::command]
#[specta::specta]
pub fn save_collection(
    state: State<'_, AppState>,
    id: String,
    name: String,
    query: BookQuery,
) -> AppResult<CollectionDto> {
    Ok(state.library()?.save_collection(&id, &name, &query)?.into())
}

#[tauri::command]
#[specta::specta]
pub fn delete_collection(state: State<'_, AppState>, id: String) -> AppResult<()> {
    Ok(state.library()?.delete_collection(&id)?)
}
