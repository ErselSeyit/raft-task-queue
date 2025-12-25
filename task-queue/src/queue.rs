use crate::error::TaskQueueError;
use shared::types::{Task, TaskStatus, TaskResult};
use uuid::Uuid;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

pub struct TaskQueue {
    tasks: Arc<RwLock<HashMap<Uuid, Task>>>,
    pending_tasks: Arc<RwLock<Vec<Uuid>>>,
    max_queue_size: usize,
}

impl TaskQueue {
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            pending_tasks: Arc::new(RwLock::new(Vec::new())),
            max_queue_size,
        }
    }
    
    pub async fn submit_task(&self, name: String, payload: Vec<u8>, max_retries: u32) -> Result<Uuid, TaskQueueError> {
        let mut tasks_guard = self.tasks.write().await;
        let mut pending_guard = self.pending_tasks.write().await;
        
        if tasks_guard.len() >= self.max_queue_size {
            return Err(TaskQueueError::QueueFull);
        }
        
        let task = Task {
            id: Uuid::new_v4(),
            name,
            payload,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            scheduled_at: None,
            completed_at: None,
            retries: 0,
            max_retries,
        };
        
        let task_id = task.id;
        pending_guard.push(task_id);
        tasks_guard.insert(task_id, task);
        
        info!("Submitted task: {}", task_id);
        Ok(task_id)
    }
    
    pub async fn get_task(&self, task_id: Uuid) -> Result<Task, TaskQueueError> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard.get(&task_id)
            .cloned()
            .ok_or_else(|| TaskQueueError::TaskNotFound(task_id.to_string()))
    }
    
    pub async fn get_pending_task(&self) -> Option<Task> {
        let mut pending_guard = self.pending_tasks.write().await;
        
        while let Some(task_id) = pending_guard.pop() {
            let tasks_guard = self.tasks.read().await;
            if let Some(task) = tasks_guard.get(&task_id).cloned() {
                if task.status == TaskStatus::Pending {
                    drop(tasks_guard);
                    drop(pending_guard);
                    return Some(task);
                }
            }
        }
        
        None
    }
    
    pub async fn mark_running(&self, task_id: Uuid) -> Result<(), TaskQueueError> {
        let mut tasks_guard = self.tasks.write().await;
        if let Some(task) = tasks_guard.get_mut(&task_id) {
            task.status = TaskStatus::Running;
            task.scheduled_at = Some(Utc::now());
            Ok(())
        } else {
            Err(TaskQueueError::TaskNotFound(task_id.to_string()))
        }
    }
    
    pub async fn complete_task(&self, result: TaskResult) -> Result<(), TaskQueueError> {
        let mut tasks_guard = self.tasks.write().await;
        if let Some(task) = tasks_guard.get_mut(&result.task_id) {
            if result.success {
                task.status = TaskStatus::Completed;
                task.completed_at = Some(Utc::now());
                info!("Task {} completed successfully", result.task_id);
            } else {
                if task.retries < task.max_retries {
                    task.retries += 1;
                    task.status = TaskStatus::Pending;
                    let mut pending_guard = self.pending_tasks.write().await;
                    pending_guard.push(task.id);
                    warn!("Task {} failed, retrying ({}/{})", result.task_id, task.retries, task.max_retries);
                } else {
                    task.status = TaskStatus::Failed;
                    task.completed_at = Some(Utc::now());
                    warn!("Task {} failed after {} retries", result.task_id, task.max_retries);
                }
            }
            Ok(())
        } else {
            Err(TaskQueueError::TaskNotFound(result.task_id.to_string()))
        }
    }
    
    pub async fn cancel_task(&self, task_id: Uuid) -> Result<(), TaskQueueError> {
        let mut tasks_guard = self.tasks.write().await;
        if let Some(task) = tasks_guard.get_mut(&task_id) {
            if task.status == TaskStatus::Pending || task.status == TaskStatus::Running {
                task.status = TaskStatus::Cancelled;
                task.completed_at = Some(Utc::now());
                Ok(())
            } else {
                Err(TaskQueueError::TaskNotFound(task_id.to_string()))
            }
        } else {
            Err(TaskQueueError::TaskNotFound(task_id.to_string()))
        }
    }
    
    pub async fn list_tasks(&self, status_filter: Option<TaskStatus>) -> Vec<Task> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard.values()
            .filter(|task| {
                status_filter.as_ref()
                    .map(|filter| task.status == *filter)
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }
    
    pub async fn get_stats(&self) -> HashMap<String, usize> {
        let tasks_guard = self.tasks.read().await;
        let mut stats = HashMap::new();
        
        for task in tasks_guard.values() {
            let status_key = format!("{:?}", task.status);
            *stats.entry(status_key).or_insert(0) += 1;
        }
        
        stats
    }
    
    pub async fn clear_all(&self) {
        let mut tasks_guard = self.tasks.write().await;
        let mut pending_guard = self.pending_tasks.write().await;
        tasks_guard.clear();
        pending_guard.clear();
        info!("Cleared all tasks from queue");
    }
}

