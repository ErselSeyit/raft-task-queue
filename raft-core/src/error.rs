use thiserror::Error;

#[derive(Error, Debug)]
pub enum RaftError {
    #[error("Invalid term: {0}")]
    InvalidTerm(u64),
    
    #[error("Not leader")]
    NotLeader,
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
}

