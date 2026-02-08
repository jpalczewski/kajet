use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

pub struct VaultWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

impl VaultWatcher {
    pub fn start(vault_path: &Path, tx: mpsc::UnboundedSender<Vec<PathBuf>>) -> Result<Self> {
        let mut debouncer = new_debouncer(
            Duration::from_secs(2),
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    let md_paths: Vec<PathBuf> = events
                        .iter()
                        .flat_map(|e| e.paths.iter())
                        .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
                        .cloned()
                        .collect();
                    if !md_paths.is_empty() {
                        let _ = tx.send(md_paths);
                    }
                }
                Err(errors) => {
                    for error in errors {
                        tracing::warn!("File watch error: {error}");
                    }
                }
            },
        )?;

        debouncer.watch(vault_path, RecursiveMode::Recursive)?;

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
