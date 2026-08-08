//! App-window screenshot capture (Linux).
//!
//! Uses WebKitGTK's `webkit_web_view_get_snapshot` to render the app's own
//! webview into a PNG — no screen capture, no external tools. Every GTK call
//! runs on the GTK main thread via `glib::MainContext::invoke`.

use std::sync::mpsc;
use std::time::Duration;

#[tauri::command]
pub fn capture_app_screenshot(app: tauri::AppHandle) -> Result<String, String> {
    capture_core(&app)
}

/// Sync core used by the command and the dev marker-file hook.
pub(crate) fn capture_core(app: &tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        linux::capture(app)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Err("screenshot capture is only implemented on Linux for now".into())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gtk::prelude::*;
    use std::os::raw::{c_int, c_uint, c_void};
    use tauri::Manager;
    use webkit2gtk::{gio, glib};

    fn shots_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("screenshots");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    fn find_webview(root: &gtk::Container) -> Option<webkit2gtk::WebView> {
        for child in root.children() {
            if child.type_().name().to_string() == "WebKitWebView" {
                if let Some(wv) = child.downcast_ref::<webkit2gtk::WebView>() {
                    return Some(wv.clone());
                }
            }
            if let Some(container) = child.downcast_ref::<gtk::Container>() {
                if let Some(found) = find_webview(container) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// CAIRO_STATUS_SUCCESS is 0 in cairo-sys-rs 0.18 (no named const exposed).
    const CAIRO_STATUS_SUCCESS: c_int = 0;

    unsafe extern "C" fn png_writer(
        closure: *mut c_void,
        data: *mut u8,
        length: c_uint,
    ) -> cairo_sys::cairo_status_t {
        let buf = &mut *(closure as *mut Vec<u8>);
        buf.extend_from_slice(std::slice::from_raw_parts(
            data as *const u8,
            length as usize,
        ));
        CAIRO_STATUS_SUCCESS
    }

    unsafe extern "C" fn snapshot_ready(
        source_object: *mut glib::gobject_ffi::GObject,
        result: *mut gio::ffi::GAsyncResult,
        user_data: glib::ffi::gpointer,
    ) {
        let sender = Box::from_raw(user_data as *mut std::sync::mpsc::Sender<Result<Vec<u8>, String>>);
        let webview = source_object as *mut webkit2gtk_sys::WebKitWebView;
        let out: Result<Vec<u8>, String> = (|| {
            let mut error: *mut glib::ffi::GError = std::ptr::null_mut();
            let surface =
                webkit2gtk_sys::webkit_web_view_get_snapshot_finish(webview, result, &mut error);
            if surface.is_null() {
                let msg = if error.is_null() {
                    "webview snapshot returned no surface".to_string()
                } else {
                    let msg = std::ffi::CStr::from_ptr((*error).message)
                        .to_string_lossy()
                        .into_owned();
                    glib::ffi::g_error_free(error);
                    msg
                };
                return Err(msg);
            }
            let mut bytes: Vec<u8> = Vec::new();
            let status = cairo_sys::cairo_surface_write_to_png_stream(
                surface,
                Some(png_writer),
                &mut bytes as *mut Vec<u8> as *mut c_void,
            );
            cairo_sys::cairo_surface_destroy(surface);
            if status != CAIRO_STATUS_SUCCESS {
                return Err(format!("cairo png export failed (status {status})"));
            }
            Ok(bytes)
        })();
        let _ = sender.send(out);
    }

    pub fn capture(app: &tauri::AppHandle) -> Result<String, String> {
        let dir = shots_dir(app)?;
        let path = dir.join(format!(
            "pragma-{}.png",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ));

        let (tx, rx) = mpsc::channel::<Result<Vec<u8>, String>>();
        let app = app.clone();

        // Locate the webview + kick off the async snapshot on the GTK main thread.
        glib::MainContext::default().invoke(move || {
            let webview = (|| {
                let window = app
                    .get_webview_window("main")
                    .ok_or_else(|| "main window not found".to_string())?;
                let vbox = window.default_vbox().map_err(|e| e.to_string())?;
                find_webview(vbox.upcast_ref::<gtk::Container>())
                    .ok_or_else(|| "WebKitWebView widget not found in window".to_string())
            })();

            match webview {
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
                Ok(wv) => {
                    let widget: &gtk::Widget = wv.upcast_ref();
                    let webview_ptr = widget.as_ptr() as *mut webkit2gtk_sys::WebKitWebView;
                    let sender: *mut mpsc::Sender<Result<Vec<u8>, String>> =
                        Box::into_raw(Box::new(tx));
                    unsafe {
                    webkit2gtk_sys::webkit_web_view_get_snapshot(
                        webview_ptr,
                        webkit2gtk_sys::WEBKIT_SNAPSHOT_REGION_VISIBLE,
                        0, // WEBKIT_SNAPSHOT_OPTIONS_NONE
                        std::ptr::null_mut(), // no cancellable
                        Some(snapshot_ready),
                        sender as _,
                    );
                    }
                }
            }
        });

        let bytes = rx.recv_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())??;
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        Ok(path.display().to_string())
    }
}