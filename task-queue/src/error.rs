use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskQueueError {
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    
    #[error("Task queue is full")]
    QueueFull,
    
    #[error("Task execution failed: {0}")]
    ExecutionFailed(String),
    
    #[error("Raft error: {0}")]
    RaftError(String),
}

