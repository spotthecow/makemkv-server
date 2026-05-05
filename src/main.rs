use std::process::Stdio;
use tokio::process::Command;

use crate::parse::Token;
use crate::reader::spawn_token_reader;

pub mod parse;
pub mod reader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .skip(1)
        .next()
        .unwrap_or("makemkvcon".to_string());

    let mut makemkv = {
        let mut cmd = Command::new(arg);
        cmd.args(["-r", "info", "disc:0"]);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.spawn()?
    };

    let stdout = makemkv.stdout.take().expect("makemkv stdout must exist");
    let (mut rx, reader) = spawn_token_reader(stdout);

    while let Some(token) = rx.recv().await {
        if let Token::Message { message, .. } = &token {
            println!("{message}");
        } else {
            println!("{token:?}");
        }
    }

    reader.await??;

    Ok(())
}
