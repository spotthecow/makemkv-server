use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use lib::{
    disc::{Disc, DiscBuilder},
    parse::Token,
    reader::spawn_token_reader,
};
use tokio::process::{Child, Command};
use tokio::sync::futures::Notified;
use tracing::{debug, info, warn};

use crate::state::{AppState, JobState, RipProgress};

const PROGRESS_MAX: u32 = 65536;

pub async fn scan_disc(makemkv: &Path, disc_index: u32) -> Result<Disc> {
    let arg = format!("disc:{disc_index}");
    info!(bin = %makemkv.display(), arg = %arg, "spawning makemkvcon info");

    let mut child = Command::new(makemkv)
        .args(["-r", "info", &arg])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn {}", makemkv.display()))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let (mut rx, reader) = spawn_token_reader(stdout);

    let mut builder = DiscBuilder::new();
    while let Some(token) = rx.recv().await {
        match token {
            Token::Message { message, .. } => debug!(target: "makemkvcon", msg = %message),
            Token::ProgressValues { .. }
            | Token::ProgressCurrentTitle { .. }
            | Token::ProgressTotalTitle { .. } => {}
            other => builder.push(other),
        }
    }

    reader.await.context("token reader join failed")??;
    let status = child.wait().await.context("waiting for makemkvcon")?;
    if !status.success() {
        warn!(?status, "makemkvcon exited non-zero");
        bail!("makemkvcon exited with status {status}");
    }

    let disc = builder.finish();
    if disc.titles.is_empty() {
        bail!("no disc detected (or disc has no readable titles)");
    }
    info!(titles = disc.titles.len(), "scan complete");
    Ok(disc)
}

pub struct RipOptions {
    pub makemkv: PathBuf,
    pub output_dir: PathBuf,
    pub disc_index: u32,
    pub selection: Vec<crate::state::TitleSelection>,
}

pub struct BackupOptions {
    pub makemkv: PathBuf,
    pub output_dir: PathBuf,
    pub disc_index: u32,
    pub disc_name: Option<String>,
}

pub enum RipOutcome {
    Done(Vec<PathBuf>),
    Cancelled,
    Failed(String),
}

pub async fn run_rip(opts: RipOptions, state: AppState) -> RipOutcome {
    let staging = match make_staging(&opts.output_dir).await {
        Ok(p) => p,
        Err(e) => return RipOutcome::Failed(e),
    };

    let mut moved_files: Vec<PathBuf> = Vec::new();
    let mut cancel_fut = Box::pin(state.cancel.notified());

    for (i, sel) in opts.selection.iter().enumerate() {
        update_progress(&state, |p| {
            p.current_title = sel.index;
            p.completed_titles = i as u32;
            p.fraction = 0.0;
            p.message = Some(format!("starting title {}", sel.index));
        })
        .await;

        let arg_disc = format!("disc:{}", opts.disc_index);
        let arg_title = sel.index.to_string();
        let arg_out = staging.to_string_lossy().into_owned();
        let args = [
            "-r",
            "--progress=-same",
            "mkv",
            &arg_disc,
            &arg_title,
            &arg_out,
        ];

        match run_makemkv(&opts.makemkv, &args, &state, &mut cancel_fut).await {
            Outcome::Done => {}
            Outcome::Cancelled => {
                info!("rip cancelled by user");
                return RipOutcome::Cancelled;
            }
            Outcome::Failed(e) => return RipOutcome::Failed(e),
        }

        match move_files(&staging, &opts.output_dir).await {
            Ok(mut files) => moved_files.append(&mut files),
            Err(e) => return RipOutcome::Failed(format!("move outputs: {e}")),
        }
    }

    let _ = tokio::fs::remove_dir(&staging).await;
    RipOutcome::Done(moved_files)
}

pub async fn run_backup(opts: BackupOptions, state: AppState) -> RipOutcome {
    let staging = match make_staging(&opts.output_dir).await {
        Ok(p) => p,
        Err(e) => return RipOutcome::Failed(e),
    };
    let mut cancel_fut = Box::pin(state.cancel.notified());

    update_progress(&state, |p| {
        p.message = Some("starting full disc backup".into());
    })
    .await;

    let arg_disc = format!("disc:{}", opts.disc_index);
    let arg_out = staging.to_string_lossy().into_owned();
    let args = [
        "-r",
        "--progress=-same",
        "--decrypt",
        "backup",
        &arg_disc,
        &arg_out,
    ];

    match run_makemkv(&opts.makemkv, &args, &state, &mut cancel_fut).await {
        Outcome::Done => {}
        Outcome::Cancelled => {
            info!("backup cancelled by user");
            return RipOutcome::Cancelled;
        }
        Outcome::Failed(e) => return RipOutcome::Failed(e),
    }

    let target_name = sanitize_dirname(opts.disc_name.as_deref().unwrap_or("backup"));
    let target = unique_target(&opts.output_dir, std::ffi::OsStr::new(&target_name));
    info!(from = %staging.display(), to = %target.display(), "promoting backup staging dir");
    if let Err(e) = tokio::fs::rename(&staging, &target).await {
        return RipOutcome::Failed(format!("rename staging to {}: {e}", target.display()));
    }

    RipOutcome::Done(vec![target])
}

enum Outcome {
    Done,
    Cancelled,
    Failed(String),
}

async fn run_makemkv(
    makemkv: &Path,
    args: &[&str],
    state: &AppState,
    cancel_fut: &mut Pin<Box<Notified<'_>>>,
) -> Outcome {
    info!(bin = %makemkv.display(), ?args, "spawning makemkvcon");
    let mut child = match Command::new(makemkv)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Outcome::Failed(format!("spawn makemkvcon: {e}")),
    };

    let stdout = child.stdout.take().expect("stdout piped");
    let (mut rx, reader) = spawn_token_reader(stdout);

    loop {
        tokio::select! {
            biased;
            _ = cancel_fut.as_mut() => {
                graceful_kill(&mut child).await;
                let _ = reader.await;
                return Outcome::Cancelled;
            }
            maybe = rx.recv() => match maybe {
                None => break,
                Some(token) => apply_token(token, state).await,
            }
        }
    }

    if let Err(e) = reader.await {
        warn!(error = %e, "token reader panicked");
    }
    match child.wait().await {
        Ok(status) if status.success() => Outcome::Done,
        Ok(status) => Outcome::Failed(format!("makemkvcon exited {status}")),
        Err(e) => Outcome::Failed(format!("waiting for child: {e}")),
    }
}

async fn apply_token(token: Token, state: &AppState) {
    match token {
        Token::Message { message, .. } => {
            debug!(target: "makemkvcon", msg = %message);
            update_progress(state, |p| p.message = Some(message)).await;
        }
        Token::ProgressValues { current, max, .. } => {
            let denom = if max == 0 { PROGRESS_MAX } else { max };
            let frac = (current as f32 / denom as f32).clamp(0.0, 1.0);
            update_progress(state, |p| p.fraction = frac).await;
        }
        Token::ProgressCurrentTitle { name, .. } | Token::ProgressTotalTitle { name, .. } => {
            update_progress(state, |p| p.message = Some(name)).await;
        }
        _ => {}
    }
}

async fn update_progress<F: FnOnce(&mut RipProgress)>(state: &AppState, f: F) {
    {
        let mut guard = state.job.lock().await;
        if let JobState::Ripping { progress, .. } = &mut *guard {
            f(progress);
        } else {
            return;
        }
    }
    state.bump();
}

async fn make_staging(output_dir: &Path) -> Result<PathBuf, String> {
    let job_id = chrono_like_id();
    let staging = output_dir.join(format!(".staging-{job_id}"));
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|e| format!("create staging dir: {e}"))?;
    Ok(staging)
}

async fn move_files(staging: &Path, output_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut moved = Vec::new();
    let mut entries = tokio::fs::read_dir(staging).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ft = entry.file_type().await?;
        if !ft.is_file() {
            warn!(path = %entry.path().display(), "skipping non-file in staging dir");
            continue;
        }
        let target = unique_target(output_dir, &entry.file_name());
        info!(from = %entry.path().display(), to = %target.display(), "moving rip output");
        tokio::fs::rename(entry.path(), &target).await?;
        moved.push(target);
    }
    Ok(moved)
}

fn unique_target(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or(name).to_os_string();
    let ext = path.extension();
    for i in 1u32.. {
        let mut new_name = OsString::new();
        new_name.push(&stem);
        new_name.push(format!("_{i}"));
        if let Some(e) = ext {
            new_name.push(".");
            new_name.push(e);
        }
        let candidate = dir.join(&new_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn sanitize_dirname(s: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = mapped.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "backup".to_string()
    } else {
        trimmed
    }
}

async fn graceful_kill(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .is_ok()
        {
            return;
        }
        warn!(pid, "child did not exit on SIGTERM, sending SIGKILL");
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn chrono_like_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}
