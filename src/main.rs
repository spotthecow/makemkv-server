use std::process::Stdio;
use tokio::process::Command;

use makemkv_server::disc::{DiscBuilder, Stream};
use makemkv_server::parse::Token;
use makemkv_server::reader::spawn_token_reader;

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

    let mut builder = DiscBuilder::new();

    while let Some(token) = rx.recv().await {
        if let Token::Message { message, .. } = &token {
            println!("[msg] {message}");
        } else {
            builder.push(token);
        }
    }
    let disc = builder.finish();

    for (i, title) in disc.titles.iter().enumerate() {
        println!("Title {i} ({})", title.duration.as_ref().unwrap());
        for (j, stream) in title.streams.iter().enumerate() {
            println!("\tStream {j}:");
            match stream {
                Stream::Video(video_stream) => {
                    println!("\t\tVideo: {}", video_stream.codec_long.as_ref().unwrap());
                }
                Stream::Audio(audio_stream) => {
                    println!("\t\tAudio: {}", audio_stream.codec_long.as_ref().unwrap());
                }
                Stream::Subtitle(subtitle_stream) => {
                    println!(
                        "\t\tSubtitle: {}",
                        subtitle_stream.codec_long.as_ref().unwrap()
                    );
                }
            }
        }
    }

    reader.await??;

    Ok(())
}
