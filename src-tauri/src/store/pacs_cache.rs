//! Local cache of the PACS services list returned by Aurabox.
//!
//! Persisted to the same `store.json` plugin-store as the rest of Bounce's
//! settings, under the key `pacs_services`. The PACS tab reads from this
//! cache synchronously on mount so the list survives restarts and brief
//! Aurabox outages; the Refresh button (and startup probe) write through
//! the cache after a successful `fetch_services()` call.

use crate::query::models::DicomService;
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "store.json";
const CACHE_KEY: &str = "pacs_services";

/// Read the cached PACS services list. Returns an empty list if the cache
/// is missing, malformed, or the underlying store cannot be opened — the
/// UI treats absence and emptiness identically.
pub fn load_cached_services(app: &AppHandle) -> Vec<DicomService> {
    let store = match app.store(STORE_FILE) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    match store.get(CACHE_KEY) {
        Some(value) => parse_services(&value).unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Write the PACS services list to the cache, replacing any previous value.
/// Errors from the underlying store are returned as a string suitable for
/// surfacing to the UI.
pub fn save_cached_services(
    app: &AppHandle,
    services: &[DicomService],
) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open settings store: {}", e))?;

    let value = serde_json::to_value(services)
        .map_err(|e| format!("Failed to serialise PACS services: {}", e))?;

    store.set(CACHE_KEY, value);
    store
        .save()
        .map_err(|e| format!("Failed to persist PACS services: {}", e))?;

    Ok(())
}

fn parse_services(value: &Value) -> Option<Vec<DicomService>> {
    serde_json::from_value::<Vec<DicomService>>(value.clone()).ok()
}
