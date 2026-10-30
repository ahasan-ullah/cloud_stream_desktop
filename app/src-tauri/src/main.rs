// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod downloads;
mod engine;
mod mpv;
mod proxy;
mod thumbs;

use std::sync::Arc;

use tauri::{Manager, RunEvent};

use downloads::Downloads;
use engine::Engine;
use proxy::Proxy;
use thumbs::Thumbnails;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(Thumbnails::new()))
        .register_asynchronous_uri_scheme_protocol("thumb", |ctx, request, responder| {
            thumbs::handle(ctx.app_handle(), request, responder)
        })
        .setup(|app| {
            // Downloads default to Videos/CloudStream until the user picks another folder.
            let data_dir = app.path().app_data_dir()?;
            let default_dir = app
                .path()
                .video_dir()
                .map(|d| d.join("CloudStream"))
                .unwrap_or_else(|_| data_dir.join("downloads"));
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Downloads::load(
                default_dir,
                data_dir.join("downloads.json"),
                data_dir.join("download-folder.txt"),
            ));

            let proxy = tauri::async_runtime::block_on(Proxy::start())?;
            app.manage(proxy);

            let engine = Arc::new(Engine::new());
            app.manage(Arc::clone(&engine));
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move { engine.start(&handle).await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::engine_status,
            commands::engine_restart,
            commands::engine_request,
            commands::proxy_url,
            commands::open_in_mpv,
            commands::open_url,
            commands::downloads_list,
            commands::download_start,
            commands::download_pause,
            commands::download_resume,
            commands::download_delete,
            commands::download_playback,
            commands::download_dir,
            commands::choose_download_dir,
            commands::reveal_download,
            commands::save_backup,
            commands::reveal_backups,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build CloudStream Desktop")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                let engine = Arc::clone(&app.state::<Arc<Engine>>());
                tauri::async_runtime::block_on(engine.stop());
            }
        });
}
