//! The Tauri shell.
//!
//! This crate only wires things together: it builds the app, holds shared
//! state, exposes thin commands and streams files through `book://`.
//! Anything that is a rule about books or libraries belongs in a
//! `libreri-*` crate (see docs/code-structure.md).

mod backup_store;
mod commands;
mod dto;
mod error;
mod events;
mod online_store;
mod protocol;
mod settings_store;
mod state;

use state::AppState;
use tauri::Manager;

/// Builds the typed command/event registry shared by the app and by the
/// TypeScript bindings generator.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::app::app_info,
            commands::app::open_external_url,
            commands::settings::get_settings,
            commands::settings::set_theme,
            commands::settings::set_accent,
            commands::settings::forget_recent_library,
            commands::details::details_query,
            commands::details::find_details,
            commands::details::apply_details,
            commands::details::cover_preview,
            commands::details::fill_missing_details,
            commands::details::get_online_settings,
            commands::details::set_online_settings,
            commands::details::set_source_key,
            commands::scan::scan_picture,
            commands::scan::scan_picture_file,
            commands::scan::start_phone_scan,
            commands::scan::stop_phone_scan,
            commands::library::create_library,
            commands::library::open_library,
            commands::library::close_library,
            commands::library::current_library,
            commands::library::inspect_folder,
            commands::library::rescan_library,
            commands::library::rebuild_library_index,
            commands::library::cancel_job,
            commands::books::list_books,
            commands::books::get_book,
            commands::books::library_facets,
            commands::books::update_book,
            commands::books::set_book_state,
            commands::books::move_books,
            commands::books::trash_books,
            commands::books::save_cover,
            commands::books::open_book_externally,
            commands::books::reveal_book,
            commands::folders::list_folders,
            commands::folders::create_folder,
            commands::folders::rename_folder,
            commands::folders::move_folder,
            commands::folders::trash_folder,
            commands::folders::reveal_folder,
            commands::folders::import_paths,
            commands::reader::get_position,
            commands::reader::save_position,
            commands::reader::list_annotations,
            commands::reader::save_annotation,
            commands::reader::delete_annotation,
            commands::reader::get_notebook,
            commands::reader::save_notebook,
            commands::reader::get_session,
            commands::reader::save_session,
            commands::profiles::list_profiles,
            commands::profiles::current_session,
            commands::profiles::sign_in,
            commands::profiles::sign_out,
            commands::profiles::create_profile,
            commands::profiles::update_profile,
            commands::profiles::set_profile_pin,
            commands::profiles::recover_owner,
            commands::profiles::delete_profile,
            commands::profiles::set_allowed_folders,
            commands::profiles::save_prefs,
            commands::profiles::list_collections,
            commands::profiles::save_collection,
            commands::profiles::delete_collection,
            commands::organize::bulk_edit_books,
            commands::organize::rename_tag,
            commands::organize::merge_tags,
            commands::organize::delete_tag,
            commands::organize::rename_category,
            commands::organize::delete_category,
            commands::organize::similar_tags,
            commands::notes::list_all_notes,
            commands::notes::list_notebooks,
            commands::notes::read_note,
            commands::notes::write_note,
            commands::notes::create_note,
            commands::notes::reveal_notes_folder,
            commands::notes::library_storage,
            commands::portability::export_books,
            commands::portability::copy_citation,
            commands::portability::inspect_archive,
            commands::portability::import_archive,
            commands::portability::restore_library,
            commands::portability::health_check,
            commands::portability::repair_health,
            commands::portability::locate_file,
            commands::portability::get_backup_settings,
            commands::portability::set_backup_settings,
            commands::portability::back_up_now,
            commands::portability::reveal_path,
            commands::reader::measure_scales,
            commands::reader::read_picture,
            commands::reader::export_marked_up,
            commands::search::search_text,
            commands::search::search_in_book,
            commands::search::text_status,
            commands::search::search_index_status,
            commands::search::rebuild_search_index,
            commands::search::update_search_index,
            commands::search::make_searchable,
            commands::search::forget_ocr,
            commands::search::books_without_text,
            commands::search::ocr_pages,
            commands::search::ocr_languages,
            commands::search::set_ocr_languages,
            commands::search::download_ocr_language,
            commands::search::remove_ocr_language,
            commands::pages::open_pages,
            commands::pages::page_words,
            commands::pages::page_texts,
            commands::pages::helpers_status,
            commands::pages::install_helper,
            commands::pages::page_cache_size,
            commands::pages::clear_page_cache,
            commands::portability::inspect_foreign,
            commands::portability::import_foreign,
        ])
        .events(tauri_specta::collect_events![
            events::JobEventPayload,
            events::LibraryChanged,
            events::ImportFinished,
            events::SessionChanged,
            events::DetailsFilled,
            events::PhoneScan,
            events::ExportFinished,
            events::ArchiveImported,
            events::BackupFinished,
            events::ForeignImported,
            events::HelperInstall,
            events::SearchIndexProgress,
            events::OcrFinished,
            events::OcrLanguageDownload,
        ])
}

/// Starts the desktop app.
pub fn run() {
    let builder = specta_builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .register_asynchronous_uri_scheme_protocol("book", protocol::handle)
        .setup(move |app| {
            builder.mount_events(app);
            let state = AppState::initialise(app.handle())?;
            app.manage(state);
            app.state::<AppState>().reopen_last_library();
            state::start_scheduler(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // When the last window closes, close the library cleanly so
                // the database is checkpointed and the lock released.
                let others = window
                    .app_handle()
                    .webview_windows()
                    .keys()
                    .any(|label| label != window.label());
                if !others {
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        state.close_library();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Libreri");
}

#[cfg(test)]
mod tests {
    /// Regenerates `src/lib/ipc/bindings.ts`. CI fails if the committed file
    /// is out of date, so Rust and TypeScript types never drift apart.
    #[test]
    fn export_typescript_bindings() {
        super::specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .header("// Generated by tauri-specta from the Rust commands. Do not edit.\n// Regenerate with: pnpm gen:ipc\n"),
                "../src/lib/ipc/bindings.ts",
            )
            .expect("failed to export TypeScript bindings");
    }
}
