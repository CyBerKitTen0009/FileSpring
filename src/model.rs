use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Copy,
    Move,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub action: Action,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub identity: String,
    pub bytes: u64,
    pub modified_ns: u128,
    pub hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanState {
    Prepared,
    Running,
    Completed,
    Failed,
    PartiallyCompleted,
    RollingBack,
    RolledBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemState {
    Pending,
    Staging,
    Staged,
    Published,
    Completed,
    Reverting,
    Restored,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub request: Request,
    pub expected: Snapshot,
    pub state: ItemState,
    pub stage: PathBuf,
    pub stage_identity: Option<String>,
    pub staged: Option<Snapshot>,
    pub undo_stage: PathBuf,
    pub undo_identity: Option<String>,
    pub undo_snapshot: Option<Snapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub created_ms: u128,
    pub state: PlanState,
    pub rollback_requested: bool,
    pub items: Vec<Item>,
    pub conflicts: Vec<String>,
    pub error: Option<String>,
}

impl Plan {
    pub fn bytes(&self) -> u64 {
        self.items.iter().map(|i| i.expected.bytes).sum()
    }
    pub fn completed(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.state == ItemState::Completed)
            .count()
    }
    pub fn executable(&self) -> bool {
        self.conflicts.is_empty() && !self.items.is_empty()
    }
}
