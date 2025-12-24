use async_trait::async_trait;
use crate::error::RaftError;
use shared::types::{Term, Index, LogEntry};

#[async_trait]
pub trait Storage: Send + Sync {
    async fn get_term(&self) -> Result<Term, RaftError>;
    async fn set_term(&mut self, term: Term) -> Result<(), RaftError>;
    
    async fn get_voted_for(&self) -> Result<Option<u64>, RaftError>;
    async fn set_voted_for(&mut self, voted_for: Option<u64>) -> Result<(), RaftError>;
    
    async fn get_log(&self) -> Result<Vec<(Term, LogEntry)>, RaftError>;
    async fn append_log(&mut self, entries: Vec<(Term, LogEntry)>) -> Result<(), RaftError>;
    async fn truncate_log(&mut self, from_index: Index) -> Result<(), RaftError>;
    
    async fn get_commit_index(&self) -> Result<Index, RaftError>;
    async fn set_commit_index(&mut self, index: Index) -> Result<(), RaftError>;
}

pub struct InMemoryStorage {
    term: Term,
    voted_for: Option<u64>,
    log: Vec<(Term, LogEntry)>,
    commit_index: Index,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self {
            term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
        }
    }
}

#[async_trait]
impl Storage for InMemoryStorage {
    async fn get_term(&self) -> Result<Term, RaftError> {
        Ok(self.term)
    }
    
    async fn set_term(&mut self, term: Term) -> Result<(), RaftError> {
        self.term = term;
        Ok(())
    }
    
    async fn get_voted_for(&self) -> Result<Option<u64>, RaftError> {
        Ok(self.voted_for)
    }
    
    async fn set_voted_for(&mut self, voted_for: Option<u64>) -> Result<(), RaftError> {
        self.voted_for = voted_for;
        Ok(())
    }
    
    async fn get_log(&self) -> Result<Vec<(Term, LogEntry)>, RaftError> {
        Ok(self.log.clone())
    }
    
    async fn append_log(&mut self, entries: Vec<(Term, LogEntry)>) -> Result<(), RaftError> {
        self.log.extend(entries);
        Ok(())
    }
    
    async fn truncate_log(&mut self, from_index: Index) -> Result<(), RaftError> {
        if from_index <= self.log.len() as Index {
            self.log.truncate(from_index as usize);
        }
        Ok(())
    }
    
    async fn get_commit_index(&self) -> Result<Index, RaftError> {
        Ok(self.commit_index)
    }
    
    async fn set_commit_index(&mut self, index: Index) -> Result<(), RaftError> {
        self.commit_index = index;
        Ok(())
    }
}

