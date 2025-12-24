use async_trait::async_trait;
use crate::error::RaftError;
use shared::messages::RaftMessage;
use shared::types::NodeId;

#[async_trait]
pub trait Network: Send + Sync {
    async fn send(&self, node_id: NodeId, message: RaftMessage) -> Result<(), RaftError>;
    async fn receive(&self) -> Result<(NodeId, RaftMessage), RaftError>;
}

