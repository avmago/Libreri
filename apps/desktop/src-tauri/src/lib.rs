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
mod spell_cache;
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
            commands::app::print_window,
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
            commands::edit::edit_pages,
            commands::edit::save_pages_as_book,
            commands::edit::save_filled_form,
            commands::edit::save_markup_into_pdf,
            commands::edit::list_versions,
            commands::edit::restore_version,
            commands::edit::delete_version,
            commands::edit::save_version_copy,
            commands::edit::versions_usage,
            commands::edit::delete_all_versions,
            commands::edit::start_phone_pages,
            commands::edit::pdf_page_count,
            commands::compare::start_compare,
            commands::compare::get_comparison,
            commands::compare::export_compare_report,
            commands::compare::close_compare,
            commands::listening::audio_info,
            commands::listening::get_audio_link,
            commands::listening::set_audio_link,
            commands::listening::set_sync_points,
            commands::listening::audiobooks_for,
            commands::listening::system_voices,
            commands::listening::system_speak,
            commands::listening::system_stop_speaking,
            commands::speech::speech_settings,
            commands::speech::set_speech_settings,
            commands::speech::download_speech_model,
            commands::speech::cancel_speech_model_download,
            commands::speech::remove_speech_model,
            commands::speech::transcribe_pcm,
            commands::speech::save_voice_note,
            commands::speech::transcribe_voice_note,
            commands::speech::auto_sync_audiobook,
            commands::capture::capture_add,
            commands::capture::capture_add_file,
            commands::capture::capture_preview,
            commands::capture::capture_save,
            commands::capture::capture_discard,
            commands::capture::open_note_file,
            commands::canvas::canvases,
            commands::canvas::create_canvas,
            commands::canvas::read_canvas,
            commands::canvas::write_canvas,
            commands::canvas::set_canvas_paper,
            commands::canvas::rename_canvas,
            commands::canvas::delete_canvas,
            commands::canvas::ink_settings,
            commands::canvas::set_ink_engine,
            commands::canvas::ink_to_text,
            commands::canvas::canvas_fonts,
            commands::canvas::download_canvas_font,
            commands::canvas::cancel_canvas_font_download,
            commands::canvas::remove_canvas_font,
            commands::links::link_fetch,
            commands::links::link_save,
            commands::links::link_file,
            commands::links::link_media_url,
            commands::links::link_player,
            commands::links::link_pop_out,
            commands::links::link_open_file,
            commands::feeds::feeds_overview,
            commands::feeds::feed_items,
            commands::feeds::feed_find,
            commands::feeds::feed_add,
            commands::feeds::arxiv_categories,
            commands::feeds::suggested_feeds,
            commands::feeds::feeds_add_arxiv,
            commands::feeds::feed_change,
            commands::feeds::feed_remove,
            commands::feeds::feed_folder_add,
            commands::feeds::feed_folder_change,
            commands::feeds::feed_folder_remove,
            commands::feeds::feeds_settings_set,
            commands::feeds::feeds_refresh,
            commands::feeds::feed_item_download,
            commands::feeds::feed_items_delete,
            commands::feeds::feed_item_forget_file,
            commands::feeds::feed_items_read,
            commands::feeds::feed_all_read,
            commands::feeds::feed_item_to_library,
            commands::feeds::feeds_import_opml,
            commands::feeds::feeds_export_opml,
            commands::feeds::feeds_reveal,
            commands::feeds::feed_open_file,
            commands::feeds::podcast_index_status,
            commands::feeds::podcast_index_set,
            commands::feeds::podcast_search,
            commands::feeds::podcast_trending,
            commands::feeds::podcast_categories,
            commands::feeds::podcast_progress,
            commands::feeds::podcast_played,
            commands::feeds::podcast_queue_set,
            commands::feeds::podcast_episodes,
            commands::feeds::podcast_transcript,
            commands::feeds::podcast_write_transcript,
            commands::feeds::podcast_chapters,
            commands::maths::maths_settings,
            commands::maths::set_maths_from_pictures,
            commands::maths::download_maths_model,
            commands::maths::cancel_maths_download,
            commands::maths::remove_maths_model,
            commands::maths::maths_from_picture,
            commands::ocr_models::ocr_engines,
            commands::ocr_models::set_ocr_engine,
            commands::ocr_models::download_ocr_model,
            commands::ocr_models::cancel_ocr_model_download,
            commands::ocr_models::remove_ocr_model,
            commands::voices::natural_voices,
            commands::voices::piper_languages,
            commands::voices::piper_voices,
            commands::voices::download_voice,
            commands::voices::cancel_voice_download,
            commands::voices::remove_voice,
            commands::voices::set_voice_on,
            commands::voices::speak_natural,
            commands::voices::unload_voices,
            commands::study::study_read,
            commands::study::study_write,
            commands::study::save_calendar_file,
            commands::study::timer_tray,
            commands::study::review_read,
            commands::study::review_write,
            commands::study::export_anki,
            commands::spell::spell_dictionaries,
            commands::spell::download_dictionary,
            commands::spell::cancel_dictionary_download,
            commands::spell::remove_dictionary,
            commands::spell::spell_check,
            commands::spell::spell_suggest,
            commands::spell::spell_complete,
            commands::spell::spell_learn_book,
            commands::spell::own_words,
            commands::spell::add_own_word,
            commands::spell::remove_own_word,
        ])
        .events(tauri_specta::collect_events![
            events::JobEventPayload,
            events::LibraryChanged,
            events::ImportFinished,
            events::SessionChanged,
            events::DetailsFilled,
            events::PhoneScan,
            events::PhonePage,
            events::CompareFinished,
            events::ExportFinished,
            events::FeedsChanged,
            events::ArchiveImported,
            events::BackupFinished,
            events::ForeignImported,
            events::HelperInstall,
            events::SearchIndexProgress,
            events::OcrFinished,
            events::OcrLanguageDownload,
            events::SpeechModelDownload,
            events::AutoSyncFinished,
            events::DictionaryDownload,
            events::CanvasFontDownload,
            events::MathsDownload,
            events::OcrModelDownload,
            events::VoiceDownload,
            events::TimerTrayAction,
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
