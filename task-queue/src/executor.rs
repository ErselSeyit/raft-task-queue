use crate::queue::TaskQueue;
use shared::types::{Task, TaskResult};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info, error};

pub struct TaskExecutor {
    queue: Arc<TaskQueue>,
    concurrency_limit: usize,
    semaphore: Arc<Semaphore>,
}

impl TaskExecutor {
    pub fn new(queue: Arc<TaskQueue>, concurrency_limit: usize) -> Self {
        Self {
            queue,
            concurrency_limit,
            semaphore: Arc::new(Semaphore::new(concurrency_limit)),
        }
    }
    
    pub async fn start(&self) {
        info!("Starting task executor with concurrency limit: {}", self.concurrency_limit);
        
        loop {
            if let Some(task) = self.queue.get_pending_task().await {
                let permit = self.semaphore.clone().acquire_owned().await.unwrap();
                let queue_clone = Arc::clone(&self.queue);
                let task_id = task.id;
                
                // Mark task as running
                if let Err(e) = queue_clone.mark_running(task_id).await {
                    error!("Failed to mark task as running: {}", e);
                    drop(permit);
                    continue;
                }
                
                tokio::spawn(async move {
                    let result = Self::execute_task(task).await;
                    drop(permit);
                    
                    if let Err(e) = queue_clone.complete_task(result).await {
                        error!("Failed to complete task: {}", e);
                    }
                });
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
    
    async fn execute_task(task: Task) -> TaskResult {
        info!("Executing task: {} ({})", task.id, task.name);
        
        // Simulate task execution
        let success = Self::run_task(&task).await;
        
        TaskResult {
            task_id: task.id,
            success,
            output: if success {
                Some(format!("Task {} completed successfully", task.name).into_bytes())
            } else {
                None
            },
            error: if !success {
                Some("Task execution failed".to_string())
            } else {
                None
            },
        }
    }
    
    async fn run_task(task: &Task) -> bool {
        // Simulate different types of tasks based on name
        match task.name.as_str() {
            "compute" => {
                // Simulate computation
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                true
            }
            "io" => {
                // Simulate I/O operation
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                true
            }
            "fail" => {
                // Simulate failure
                false
            }
            _ => {
                // Default: simple echo task
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                true
            }
        }
    }
}

