use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// Spawn a background thread that watches `plugins_dir` recursively and calls
/// `on_changed` (debounced by ~350ms) whenever any file inside changes.
/// `extra_dirs` are watched too (shipped-plugin symlink targets — notify
/// does not follow symlinked directories by default).
/// Ignore editor/SCM/build noise — plugin source repos are often inside git
/// trees with node_modules / target dirs; an un-filtered watcher would storm
/// hot-reloads on every git index write or build artifact touch.
fn is_ignored(p: &Path) -> bool {
    p.components().any(|c| {
        let n = c.as_os_str();
        n == "node_modules"
            || n == ".git"
            || n == "target"
            || n == ".svelte-kit"
            || n == "dist"
            || n == "build"
            || n == ".idea"
            || n == ".vscode"
    })
}

pub fn spawn_plugin_watcher<F>(
    plugins_dir: impl AsRef<Path>,
    extra_dirs: Vec<std::path::PathBuf>,
    on_changed: F,
) where
    F: Fn() + Send + 'static,
{
    let dir = plugins_dir.as_ref().to_path_buf();
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher: RecommendedWatcher =
            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                let _ = tx.send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[plugins] watcher init failed: {e}");
                    return;
                }
            };
        let Ok(_) = watcher.watch(&dir, RecursiveMode::Recursive) else {
            eprintln!("[plugins] cannot watch {}", dir.display());
            return;
        };
        eprintln!("[plugins] watching {}", dir.display());
        for extra in extra_dirs {
            if extra.exists() && watcher.watch(&extra, RecursiveMode::Recursive).is_ok() {
                eprintln!("[plugins] also watching symlink target {}", extra.display());
            }
        }

        let mut pending = false;
        loop {
            let mut got_event = false;
            while let Ok(res) = rx.try_recv() {
                if let Ok(ref evt) = res {
                    // Ignore config.json changes — these are written by the
                    // settings UI and should not trigger a full plugin reload
                    // (which would destroy the component and lose edit state).
                    let dominated_by_config = evt
                        .paths
                        .iter()
                        .all(|p| p.file_name().map(|n| n == "config.json").unwrap_or(false));
                    let dominated_by_ignored = evt.paths.iter().all(|p| is_ignored(p));
                    if !dominated_by_config && !dominated_by_ignored {
                        got_event = true;
                    }
                }
            }
            if got_event {
                pending = true;
            }
            if pending {
                // debounce: wait for quiescence before firing
                std::thread::sleep(Duration::from_millis(350));
                let mut again = false;
                while let Ok(res) = rx.try_recv() {
                    if let Ok(ref evt) = res {
                        let dominated_by_config = evt
                            .paths
                            .iter()
                            .all(|p| p.file_name().map(|n| n == "config.json").unwrap_or(false));
                        let dominated_by_ignored = evt.paths.iter().all(|p| is_ignored(p));
                        if !dominated_by_config && !dominated_by_ignored {
                            again = true;
                        }
                    }
                }
                if !again {
                    eprintln!("[plugins] change detected — hot reload");
                    on_changed();
                    pending = false;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}
