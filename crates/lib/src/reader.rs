use std::io;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::ChildStdout,
    sync::mpsc,
    task::JoinHandle,
};

use crate::parse::{ParseError, Token, parse_line};

pub fn spawn_token_reader(
    stdout: ChildStdout,
) -> (mpsc::Receiver<Token>, JoinHandle<io::Result<()>>) {
    let (tx, rx) = mpsc::channel(64);
    let handle = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await? {
                None => break,
                Some(line) => match parse_line(&line) {
                    Ok(token) => {
                        if tx.send(token).await.is_err() {
                            break;
                        }
                    }
                    Err(ParseError::NotAToken | ParseError::UnknownKind(_)) => {}
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        panic!("parse error on {line:?}: {e}");
                        #[cfg(not(debug_assertions))]
                        eprintln!("parse error on {line:?}: {e}");
                    }
                },
            }
        }
        Ok(())
    });
    (rx, handle)
}
