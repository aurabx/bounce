



use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

#[derive(Debug, Clone)]
pub struct Records {
    app_handle: AppHandle,
}

impl Records {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
        }
    }

    pub async fn db(&self) -> Result<DbPool, String> {
        let instances = self.app_handle.state::<DbInstances>();
        let instances = instances.0.read().await;

        // Clone the pool instead of returning a reference to it
        match instances.get("sqlite:bounce.db") {
            Some(pool) => Ok(*pool.clone()),
            None => Err("Database connection not found".to_string()),
        }
    }
}