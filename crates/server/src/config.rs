use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about = "MakeMKV web server")]
pub struct Config {
    #[arg(long, env = "SERVER_BIND", default_value = "127.0.0.1:8080")]
    pub bind: String,

    #[arg(long, env = "SERVER_MAKEMKV", default_value = "makemkvcon")]
    pub makemkv: PathBuf,

    #[arg(long, env = "SERVER_OUTPUT_DIR")]
    pub output_dir: PathBuf,

    #[arg(long, env = "SERVER_LOG_DIR")]
    pub log_dir: Option<PathBuf>,
}

impl Config {
    pub fn validate(self) -> Result<Self> {
        let canonical = self
            .output_dir
            .canonicalize()
            .with_context(|| format!("output-dir not accessible: {}", self.output_dir.display()))?;
        let meta = std::fs::metadata(&canonical)?;
        if !meta.is_dir() {
            bail!("output-dir is not a directory: {}", canonical.display());
        }
        let probe = canonical.join(".server-write-probe");
        std::fs::write(&probe, b"")
            .with_context(|| format!("output-dir is not writable: {}", canonical.display()))?;
        let _ = std::fs::remove_file(&probe);

        if let Some(log_dir) = &self.log_dir {
            std::fs::create_dir_all(log_dir)
                .with_context(|| format!("could not create log-dir: {}", log_dir.display()))?;
        }

        Ok(Self {
            output_dir: canonical,
            ..self
        })
    }
}
