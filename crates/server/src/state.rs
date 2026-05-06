use std::path::PathBuf;
use std::sync::Arc;

use lib::disc::Disc;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify, watch};

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleSelection {
    pub index: u32,
    #[serde(default)]
    pub streams: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RipProgress {
    pub current_title: u32,
    pub completed_titles: u32,
    pub total_titles: u32,
    pub fraction: f32,
    pub message: Option<String>,
}

#[derive(Default)]
pub enum JobState {
    #[default]
    Idle,
    Scanning,
    Scanned(Arc<Disc>),
    Ripping {
        disc: Arc<Disc>,
        selection: Vec<TitleSelection>,
        progress: RipProgress,
        op: &'static str,
    },
    Done {
        disc: Arc<Disc>,
        files: Vec<PathBuf>,
    },
    Failed(String),
}

impl JobState {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Scanning => "scanning",
            Self::Scanned(_) => "scanned",
            Self::Ripping { .. } => "ripping",
            Self::Done { .. } => "done",
            Self::Failed(_) => "failed",
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub job: Arc<Mutex<JobState>>,
    pub cancel: Arc<Notify>,
    pub shutdown: Arc<Notify>,
    pub status_tx: watch::Sender<u64>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (status_tx, _) = watch::channel(0u64);
        Self {
            config: Arc::new(config),
            job: Arc::new(Mutex::new(JobState::Idle)),
            cancel: Arc::new(Notify::new()),
            shutdown: Arc::new(Notify::new()),
            status_tx,
        }
    }

    /// Notify all status subscribers that JobState changed.
    /// MUST be called after any mutation of `self.job`.
    pub fn bump(&self) {
        self.status_tx.send_modify(|v| *v = v.wrapping_add(1));
    }
}
