use shared::types::{NodeId, Term, Index, LogEntry};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
}

pub struct RaftState {
    pub node_id: NodeId,
    pub current_term: Term,
    pub voted_for: Option<NodeId>,
    pub log: Vec<(Term, LogEntry)>,
    pub commit_index: Index,
    pub last_applied: Index,
    pub state: NodeState,
    pub peers: Vec<NodeId>,
    
    // Leader state
    pub next_index: HashMap<NodeId, Index>,
    pub match_index: HashMap<NodeId, Index>,
}

impl RaftState {
    pub fn new(node_id: NodeId, peers: Vec<NodeId>) -> Self {
        let mut next_index = HashMap::new();
        let mut match_index = HashMap::new();
        
        for peer_id in &peers {
            next_index.insert(*peer_id, 1);
            match_index.insert(*peer_id, 0);
        }
        
        Self {
            node_id,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            state: NodeState::Follower,
            peers,
            next_index,
            match_index,
        }
    }
    
    pub fn last_log_index(&self) -> Index {
        self.log.len() as Index
    }
    
    pub fn last_log_term(&self) -> Term {
        self.log.last()
            .map(|(term, _)| *term)
            .unwrap_or(0)
    }
    
    pub fn become_follower(&mut self, term: Term) {
        self.current_term = term;
        self.voted_for = None;
        self.state = NodeState::Follower;
    }
    
    pub fn become_candidate(&mut self) -> Term {
        self.current_term += 1;
        self.state = NodeState::Candidate;
        self.voted_for = Some(self.node_id);
        self.current_term
    }
    
    pub fn become_leader(&mut self) {
        self.state = NodeState::Leader;
        let last_index = self.last_log_index();
        
        for peer_id in &self.peers {
            self.next_index.insert(*peer_id, last_index + 1);
            self.match_index.insert(*peer_id, 0);
        }
    }
    
}

