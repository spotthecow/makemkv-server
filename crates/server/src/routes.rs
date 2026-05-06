use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Sse, sse::Event, sse::KeepAlive},
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_stream::wrappers::WatchStream;
use tracing::{error, info};

use crate::{
    makemkv::{self, BackupOptions, RipOptions, RipOutcome},
    state::{AppState, JobState},
};

const DISC_INDEX: u32 = 0;

async fn build_status(state: &AppState) -> Value {
    let job = state.job.lock().await;
    match &*job {
        JobState::Ripping {
            progress,
            selection,
            op,
            ..
        } => json!({
            "state": "ripping",
            "op": op,
            "progress": progress,
            "selection": selection,
        }),
        JobState::Done { files, .. } => json!({
            "state": "done",
            "files": files.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        }),
        JobState::Failed(err) => json!({ "state": "failed", "error": err }),
        other => json!({ "state": other.kind() }),
    }
}

pub async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    Json(build_status(&state).await)
}

pub async fn get_status_stream(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.status_tx.subscribe();
    let shutdown = state.shutdown.clone();
    let body_state = state.clone();
    let stream = WatchStream::new(rx)
        .then(move |_| {
            let state = body_state.clone();
            async move {
                let body = build_status(&state).await;
                Ok(Event::default().data(body.to_string()))
            }
        })
        .take_until(async move { shutdown.notified().await });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

pub async fn post_scan(State(state): State<AppState>) -> impl IntoResponse {
    {
        let mut job = state.job.lock().await;
        match *job {
            JobState::Scanning => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "scan already in progress" })),
                )
                    .into_response();
            }
            JobState::Ripping { .. } => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "rip in progress" })),
                )
                    .into_response();
            }
            _ => *job = JobState::Scanning,
        }
    }
    state.bump();

    let bin = state.config.makemkv.clone();
    let bg_state = state.clone();
    tokio::spawn(async move {
        info!("scan started");
        match makemkv::scan_disc(&bin, DISC_INDEX).await {
            Ok(disc) => {
                {
                    let mut job = bg_state.job.lock().await;
                    *job = JobState::Scanned(Arc::new(disc));
                }
                bg_state.bump();
                info!("scan stored");
            }
            Err(err) => {
                error!(error = %err, "scan failed");
                {
                    let mut job = bg_state.job.lock().await;
                    *job = JobState::Failed(format!("{err:#}"));
                }
                bg_state.bump();
            }
        }
    });

    (StatusCode::ACCEPTED, Json(json!({ "state": "scanning" }))).into_response()
}

pub async fn get_disc(State(state): State<AppState>) -> impl IntoResponse {
    let job = state.job.lock().await;
    match &*job {
        JobState::Scanned(disc) => {
            (StatusCode::OK, Json(json!({ "disc": &**disc }))).into_response()
        }
        JobState::Ripping { disc, .. } => {
            (StatusCode::OK, Json(json!({ "disc": &**disc }))).into_response()
        }
        JobState::Done { disc, .. } => {
            (StatusCode::OK, Json(json!({ "disc": &**disc }))).into_response()
        }
        JobState::Scanning => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "scan in progress" })),
        )
            .into_response(),
        JobState::Failed(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err })),
        )
            .into_response(),
        JobState::Idle => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "no disc scanned yet" })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct RipRequest {
    pub titles: Vec<crate::state::TitleSelection>,
}

pub async fn post_rip(
    State(state): State<AppState>,
    Json(req): Json<RipRequest>,
) -> impl IntoResponse {
    if req.titles.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "no titles selected" })),
        )
            .into_response();
    }

    {
        let mut job = state.job.lock().await;
        let disc = match &*job {
            JobState::Scanned(d) | JobState::Done { disc: d, .. } => d.clone(),
            JobState::Ripping { .. } => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "rip already in progress" })),
                )
                    .into_response();
            }
            JobState::Scanning => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "scan in progress" })),
                )
                    .into_response();
            }
            JobState::Idle | JobState::Failed(_) => {
                return (
                    StatusCode::PRECONDITION_FAILED,
                    Json(json!({ "error": "no disc scanned" })),
                )
                    .into_response();
            }
        };
        for sel in &req.titles {
            if (sel.index as usize) >= disc.titles.len() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("title index {} out of range", sel.index) })),
                )
                    .into_response();
            }
        }
        *job = JobState::Ripping {
            disc,
            selection: req.titles.clone(),
            progress: crate::state::RipProgress {
                total_titles: req.titles.len() as u32,
                ..Default::default()
            },
            op: "rip",
        };
    }
    state.bump();

    let opts = RipOptions {
        makemkv: state.config.makemkv.clone(),
        output_dir: state.config.output_dir.clone(),
        disc_index: DISC_INDEX,
        selection: req.titles,
    };
    let bg_state = state.clone();

    tokio::spawn(async move {
        info!(titles = opts.selection.len(), "rip started");
        let outcome = makemkv::run_rip(opts, bg_state.clone()).await;
        let mut job = bg_state.job.lock().await;
        match outcome {
            RipOutcome::Done(files) => {
                let disc = match &*job {
                    JobState::Ripping { disc, .. } => disc.clone(),
                    _ => return,
                };
                info!(count = files.len(), "rip complete");
                *job = JobState::Done { disc, files };
            }
            RipOutcome::Cancelled => {
                *job = JobState::Failed("cancelled".into());
            }
            RipOutcome::Failed(err) => {
                error!(error = %err, "rip failed");
                *job = JobState::Failed(err);
            }
        }
        drop(job);
        bg_state.bump();
    });

    (StatusCode::ACCEPTED, Json(json!({ "state": "ripping" }))).into_response()
}

pub async fn post_backup(State(state): State<AppState>) -> impl IntoResponse {
    let disc_name = {
        let mut job = state.job.lock().await;
        let disc = match &*job {
            JobState::Scanned(d) | JobState::Done { disc: d, .. } => {
                if d.titles.is_empty() {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "disc has no titles" })),
                    )
                        .into_response();
                }
                d.clone()
            }
            JobState::Ripping { .. } => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "rip already in progress" })),
                )
                    .into_response();
            }
            JobState::Scanning => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "scan in progress" })),
                )
                    .into_response();
            }
            JobState::Idle | JobState::Failed(_) => {
                return (
                    StatusCode::PRECONDITION_FAILED,
                    Json(json!({ "error": "no disc scanned" })),
                )
                    .into_response();
            }
        };
        let name = disc.name.clone();
        *job = JobState::Ripping {
            disc,
            selection: Vec::new(),
            progress: crate::state::RipProgress {
                total_titles: 1,
                ..Default::default()
            },
            op: "backup",
        };
        name
    };
    state.bump();

    let opts = BackupOptions {
        makemkv: state.config.makemkv.clone(),
        output_dir: state.config.output_dir.clone(),
        disc_index: DISC_INDEX,
        disc_name,
    };
    let bg_state = state.clone();

    tokio::spawn(async move {
        info!("backup started");
        let outcome = makemkv::run_backup(opts, bg_state.clone()).await;
        let mut job = bg_state.job.lock().await;
        match outcome {
            RipOutcome::Done(files) => {
                let disc = match &*job {
                    JobState::Ripping { disc, .. } => disc.clone(),
                    _ => return,
                };
                info!(target = ?files.first(), "backup complete");
                *job = JobState::Done { disc, files };
            }
            RipOutcome::Cancelled => {
                *job = JobState::Failed("cancelled".into());
            }
            RipOutcome::Failed(err) => {
                error!(error = %err, "backup failed");
                *job = JobState::Failed(err);
            }
        }
        drop(job);
        bg_state.bump();
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({ "state": "ripping", "op": "backup" })),
    )
        .into_response()
}

pub async fn post_cancel(State(state): State<AppState>) -> impl IntoResponse {
    let job = state.job.lock().await;
    match &*job {
        JobState::Ripping { .. } => {
            state.cancel.notify_waiters();
            (StatusCode::ACCEPTED, Json(json!({ "ok": true }))).into_response()
        }
        _ => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "no rip in progress" })),
        )
            .into_response(),
    }
}
