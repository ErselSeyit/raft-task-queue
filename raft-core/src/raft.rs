use crate::error::RaftError;
use crate::state::{RaftState, NodeState};
use crate::storage::Storage;
use crate::network::Network;
use shared::types::{NodeId, Term, Index, LogEntry};
use shared::messages::{VoteRequest, VoteResponse, AppendEntriesRequest, AppendEntriesResponse, RaftMessage};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{info, error};

const ELECTION_TIMEOUT_MIN: u64 = 150;
const ELECTION_TIMEOUT_MAX: u64 = 300;
const HEARTBEAT_INTERVAL: u64 = 50;

pub struct RaftNode<S, N> 
where
    S: Storage,
    N: Network,
{
    state: Arc<RwLock<RaftState>>,
    storage: Arc<RwLock<S>>,
    network: Arc<N>,
    last_heartbeat: Arc<RwLock<Instant>>,
}

impl<S, N> RaftNode<S, N>
where
    S: Storage + 'static,
    N: Network + 'static,
{
    pub fn new(node_id: NodeId, peers: Vec<NodeId>, storage: S, network: N) -> Self {
        let state = Arc::new(RwLock::new(RaftState::new(node_id, peers)));
        let storage = Arc::new(RwLock::new(storage));
        let network = Arc::new(network);
        let last_heartbeat = Arc::new(RwLock::new(Instant::now()));
        
        Self {
            state,
            storage,
            network,
            last_heartbeat,
        }
    }
    
    pub async fn start(&self) -> Result<(), RaftError> {
        info!("Starting Raft node");
        
        // Load persisted state
        self.load_state().await?;
        
        // Start main event loop
        let state_clone = Arc::clone(&self.state);
        let storage_clone = Arc::clone(&self.storage);
        let network_clone = Arc::clone(&self.network);
        let heartbeat_clone = Arc::clone(&self.last_heartbeat);
        
        tokio::spawn(async move {
            Self::event_loop(state_clone, storage_clone, network_clone, heartbeat_clone).await;
        });
        
        Ok(())
    }
    
    async fn event_loop(
        state: Arc<RwLock<RaftState>>,
        storage: Arc<RwLock<S>>,
        network: Arc<N>,
        last_heartbeat: Arc<RwLock<Instant>>,
    ) {
        loop {
            tokio::select! {
                _ = Self::handle_timeout(&state, &storage, &network, &last_heartbeat) => {},
                result = Self::handle_message(&state, &storage, &network) => {
                    if let Err(e) = result {
                        error!("Error handling message: {}", e);
                    }
                }
            }
        }
    }
    
    async fn handle_timeout(
        state: &Arc<RwLock<RaftState>>,
        storage: &Arc<RwLock<S>>,
        network: &Arc<N>,
        last_heartbeat: &Arc<RwLock<Instant>>,
    ) {
        let timeout = Self::random_election_timeout();
        tokio::time::sleep(Duration::from_millis(timeout)).await;
        
        let state_guard = state.read().await;
        let heartbeat_guard = last_heartbeat.read().await;
        
        match state_guard.state {
            NodeState::Follower | NodeState::Candidate => {
                if heartbeat_guard.elapsed() > Duration::from_millis(timeout) {
                    drop(state_guard);
                    drop(heartbeat_guard);
                    Self::start_election(state, storage, network).await;
                }
            }
            NodeState::Leader => {
                drop(state_guard);
                drop(heartbeat_guard);
                Self::send_heartbeat(state, storage, network).await;
            }
        }
    }
    
    async fn handle_message(
        state: &Arc<RwLock<RaftState>>,
        storage: &Arc<RwLock<S>>,
        network: &Arc<N>,
    ) -> Result<(), RaftError> {
        let (from_id, message) = network.receive().await?;
        
        match message {
            RaftMessage::VoteRequest(req) => {
                Self::handle_vote_request(state, storage, network, from_id, req).await?;
            }
            RaftMessage::VoteResponse(resp) => {
                Self::handle_vote_response(state, storage, from_id, resp).await?;
            }
            RaftMessage::AppendEntriesRequest(req) => {
                Self::handle_append_entries(state, storage, network, from_id, req).await?;
            }
            RaftMessage::AppendEntriesResponse(resp) => {
                Self::handle_append_entries_response(state, storage, from_id, resp).await?;
            }
        }
        
        Ok(())
    }
    
    async fn start_election(
        state: &Arc<RwLock<RaftState>>,
        storage: &Arc<RwLock<S>>,
        network: &Arc<N>,
    ) {
        let mut state_guard = state.write().await;
        let term = state_guard.become_candidate();
        
        let last_log_index = state_guard.last_log_index();
        let last_log_term = state_guard.last_log_term();
        let candidate_id = state_guard.node_id;
        let peers = state_guard.peers.clone();
        drop(state_guard);
        
        let mut storage_guard = storage.write().await;
        storage_guard.set_term(term).await.unwrap();
        storage_guard.set_voted_for(Some(candidate_id)).await.unwrap();
        drop(storage_guard);
        
        info!("Starting election for term {}", term);
        
        let vote_request = VoteRequest {
            term,
            candidate_id,
            last_log_index,
            last_log_term,
        };
        
        let mut votes = 1; // Vote for self
        let mut responses = 0;
        
        for peer_id in peers {
            let network_clone = Arc::clone(network);
            let req = vote_request.clone();
            
            tokio::spawn(async move {
                if let Ok(()) = network_clone.send(peer_id, RaftMessage::VoteRequest(req)).await {
                    // Response will be handled in handle_message
                }
            });
        }
        
        // Check if we got majority
        let state_guard = state.read().await;
        let total_nodes = state_guard.peers.len() + 1;
        drop(state_guard);
        
        if votes > total_nodes / 2 {
            let mut state_guard = state.write().await;
            state_guard.become_leader();
            info!("Elected as leader for term {}", term);
        }
    }
    
    async fn send_heartbeat(
        state: &Arc<RwLock<RaftState>>,
        _storage: &Arc<RwLock<S>>,
        network: &Arc<N>,
    ) {
        let state_guard = state.read().await;
        if state_guard.state != NodeState::Leader {
            return;
        }
        
        let term = state_guard.current_term;
        let leader_id = state_guard.node_id;
        let peers = state_guard.peers.clone();
        let commit_index = state_guard.commit_index;
        drop(state_guard);
        
        for peer_id in peers {
            let network_clone = Arc::clone(network);
            let state_clone = Arc::clone(state);
            
            tokio::spawn(async move {
                let state_guard = state_clone.read().await;
                let next_idx = state_guard.next_index.get(&peer_id).copied().unwrap_or(1);
                let prev_log_index = next_idx.saturating_sub(1);
                let prev_log_term = if prev_log_index > 0 {
                    state_guard.log.get((prev_log_index - 1) as usize)
                        .map(|(term, _)| *term)
                        .unwrap_or(0)
                } else {
                    0
                };
                
                let entries: Vec<LogEntry> = state_guard.log
                    .iter()
                    .skip(prev_log_index as usize)
                    .map(|(_, entry)| entry.clone())
                    .collect();
                
                drop(state_guard);
                
                let req = AppendEntriesRequest {
                    term,
                    leader_id,
                    prev_log_index,
                    prev_log_term,
                    entries,
                    leader_commit: commit_index,
                };
                
                let _ = network_clone.send(peer_id, RaftMessage::AppendEntriesRequest(req)).await;
            });
        }
    }
    
    async fn handle_vote_request(
        state: &Arc<RwLock<RaftState>>,
        storage: &Arc<RwLock<S>>,
        network: &Arc<N>,
        from_id: NodeId,
        req: VoteRequest,
    ) -> Result<(), RaftError> {
        let mut state_guard = state.write().await;
        let mut storage_guard = storage.write().await;
        
        if req.term > state_guard.current_term {
            state_guard.become_follower(req.term);
            storage_guard.set_term(req.term).await?;
        }
        
        let vote_granted = req.term == state_guard.current_term
            && (state_guard.voted_for.is_none() || state_guard.voted_for == Some(from_id))
            && req.last_log_term >= state_guard.last_log_term()
            && req.last_log_index >= state_guard.last_log_index();
        
        if vote_granted {
            state_guard.voted_for = Some(from_id);
            storage_guard.set_voted_for(Some(from_id)).await?;
        }
        
        let term = state_guard.current_term;
        drop(state_guard);
        drop(storage_guard);
        
        let resp = VoteResponse {
            term,
            vote_granted,
        };
        
        network.send(from_id, RaftMessage::VoteResponse(resp)).await?;
        Ok(())
    }
    
    async fn handle_vote_response(
        _state: &Arc<RwLock<RaftState>>,
        _storage: &Arc<RwLock<S>>,
        _from_id: NodeId,
        _resp: VoteResponse,
    ) -> Result<(), RaftError> {
        // Handle vote response logic
        Ok(())
    }
    
    async fn handle_append_entries(
        state: &Arc<RwLock<RaftState>>,
        storage: &Arc<RwLock<S>>,
        network: &Arc<N>,
        from_id: NodeId,
        req: AppendEntriesRequest,
    ) -> Result<(), RaftError> {
        let mut state_guard = state.write().await;
        let mut storage_guard = storage.write().await;
        
        if req.term >= state_guard.current_term {
            
            if req.term > state_guard.current_term {
                state_guard.become_follower(req.term);
                storage_guard.set_term(req.term).await?;
            }
            
            let success = if req.prev_log_index == 0
                || (req.prev_log_index <= state_guard.log.len() as Index
                    && state_guard.log.get((req.prev_log_index - 1) as usize)
                        .map(|(term, _)| *term == req.prev_log_term)
                        .unwrap_or(false))
            {
                // Append entries
                if !req.entries.is_empty() {
                    let entries: Vec<(Term, LogEntry)> = req.entries
                        .into_iter()
                        .map(|e| (req.term, e))
                        .collect();
                    state_guard.log.truncate(req.prev_log_index as usize);
                    state_guard.log.extend(entries);
                    storage_guard.append_log(state_guard.log.clone()).await?;
                }
                
                if req.leader_commit > state_guard.commit_index {
                    state_guard.commit_index = req.leader_commit.min(state_guard.log.len() as Index);
                    storage_guard.set_commit_index(state_guard.commit_index).await?;
                }
                
                true
            } else {
                false
            };
            
            let term = state_guard.current_term;
            drop(state_guard);
            drop(storage_guard);
            
            let resp = AppendEntriesResponse {
                term,
                success,
                next_index: None,
            };
            
            network.send(from_id, RaftMessage::AppendEntriesResponse(resp)).await?;
        }
        
        Ok(())
    }
    
    async fn handle_append_entries_response(
        _state: &Arc<RwLock<RaftState>>,
        _storage: &Arc<RwLock<S>>,
        _from_id: NodeId,
        _resp: AppendEntriesResponse,
    ) -> Result<(), RaftError> {
        // Handle append entries response
        Ok(())
    }
    
    async fn load_state(&self) -> Result<(), RaftError> {
        let storage_guard = self.storage.read().await;
        let mut state_guard = self.state.write().await;
        
        state_guard.current_term = storage_guard.get_term().await?;
        state_guard.voted_for = storage_guard.get_voted_for().await?;
        state_guard.log = storage_guard.get_log().await?;
        state_guard.commit_index = storage_guard.get_commit_index().await?;
        
        Ok(())
    }
    
    fn random_election_timeout() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let random = (seed % (ELECTION_TIMEOUT_MAX - ELECTION_TIMEOUT_MIN)) + ELECTION_TIMEOUT_MIN;
        random
    }
    
    pub async fn propose(&self, entry: LogEntry) -> Result<Index, RaftError> {
        let state_guard = self.state.read().await;
        
        if state_guard.state != NodeState::Leader {
            return Err(RaftError::NotLeader);
        }
        
        let term = state_guard.current_term;
        let index = state_guard.last_log_index() + 1;
        drop(state_guard);
        
        let entry_clone = entry.clone();
        let mut state_guard = self.state.write().await;
        state_guard.log.push((term, entry));
        drop(state_guard);
        
        let mut storage_guard = self.storage.write().await;
        storage_guard.append_log(vec![(term, entry_clone)]).await?;
        
        Ok(index)
    }
}

