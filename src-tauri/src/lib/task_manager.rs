use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};
use crate::log_info;

#[derive(Debug)]
pub struct TaskInfo {
    pub task_handle: JoinHandle<()>,
    pub started_at: Instant,
    pub description: String,
    pub cancel_tx: Option<oneshot::Sender<()>>,
}

#[derive(Debug, Clone)]
pub struct TaskManager {
    tasks: Arc<Mutex<HashMap<String, TaskInfo>>>,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a task that will be managed by the TaskManager
    pub async fn spawn_task<F>(&self,
           task_id: String,
           description: String,
           cancellable: bool,
           future: F
    ) where
        F: FnOnce(Option<oneshot::Receiver<()>>) -> tokio::task::JoinHandle<()>,
    {
        let mut tasks = self.tasks.lock().await;

        // If there's an existing task with this ID, cancel it first
        if let Some(existing) = tasks.remove(&task_id) {
            if let Some(cancel_tx) = existing.cancel_tx {
                let _ = cancel_tx.send(());
            }
            existing.task_handle.abort();
            log_info!("Cancelled existing task: {}", task_id);
        }

        let (cancel_tx, cancel_rx) = if cancellable {
            let (tx, rx) = oneshot::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        // Spawn the new task
        let task_handle = future(cancel_rx);
        
        let logable_desc = description.clone();

        // Store the task information
        tasks.insert(task_id.clone(), TaskInfo {
            task_handle,
            started_at: Instant::now(),
            description,
            cancel_tx,
        });

        log_info!("Started task: {} - {}", task_id.clone(), logable_desc);
        
        
        
    }

    /// Cancel a specific task by ID
    pub async fn cancel_task(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.lock().await;

        if let Some(task_info) = tasks.remove(task_id) {
            if let Some(cancel_tx) = task_info.cancel_tx {
                let _ = cancel_tx.send(());
            }
            task_info.task_handle.abort();
            log_info!("Manually cancelled task: {}", task_id);
            true
        } else {
            false
        }
    }

    /// Remove completed tasks from the map
    pub async fn cleanup_completed_tasks(&self) {
        let mut tasks = self.tasks.lock().await;

        let completed_tasks: Vec<String> = tasks.iter()
            .filter(|(_, info)| info.task_handle.is_finished())
            .map(|(id, _)| id.clone())
            .collect();

        for task_id in completed_tasks {
            tasks.remove(&task_id);
            log_info!("Cleaned up completed task: {}", task_id);
        }
    }

    /// Get information about all running tasks
    pub async fn get_running_tasks(&self) -> Vec<(String, String, Duration)> {
        let tasks = self.tasks.lock().await;

        tasks.iter()
            .filter(|(_, info)| !info.task_handle.is_finished())
            .map(|(id, info)| {
                (
                    id.clone(),
                    info.description.clone(),
                    info.started_at.elapsed()
                )
            })
            .collect()
    }
}