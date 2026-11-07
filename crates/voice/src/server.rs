//! The voice process's line protocol. The app starts itself again as the
//! voice process, so that the engine's native code, should it fail, ends
//! only that process and never a running show. The app writes one
//! `Request` per line to the process's standard input; the process loads
//! the model once, renders each request into its file, and answers each
//! with one `Reply` line on its standard output, in order.

use crate::engine::Clip;
use crate::{wav, Line, VoiceError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub line: Line,
    /// The clip's file; written whole or not at all.
    pub out: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reply {
    /// The model loaded: sent once, before any clip.
    Ready,
    Written,
    Failed,
}

/// Something that says a line: the engine, or a fake in tests.
pub trait Render {
    fn render(&self, line: &Line) -> Result<Clip, VoiceError>;
}

impl Render for crate::engine::Engine {
    fn render(&self, line: &Line) -> Result<Clip, VoiceError> {
        crate::engine::Engine::render(self, line)
    }
}

/// Serves `requests` with `renderer` until they end. A line that does not
/// read as a request is answered as failed, so the app's count stays.
pub fn serve(renderer: &impl Render, requests: impl BufRead, replies: impl Write) -> std::io::Result<()> {
    let mut replies = BufWriter::new(replies);
    reply(&mut replies, Reply::Ready)?;
    for request in requests.lines() {
        let request = request?;
        let written = serde_json::from_str::<Request>(&request).ok().is_some_and(|request| write_clip(renderer, &request).is_ok());
        reply(&mut replies, if written { Reply::Written } else { Reply::Failed })?;
    }
    Ok(())
}

fn reply(replies: &mut impl Write, reply: Reply) -> std::io::Result<()> {
    serde_json::to_writer(&mut *replies, &reply)?;
    replies.write_all(b"\n")?;
    replies.flush()
}

/// Renders the request and writes it beside its file, then moves it there.
fn write_clip(renderer: &impl Render, request: &Request) -> Result<(), VoiceError> {
    let clip = renderer.render(&request.line)?;
    let partial = partial_path(&request.out);
    let written = fs::File::create(&partial).and_then(|file| {
        wav::write(BufWriter::new(&file), &clip.samples, clip.rate)?;
        file.sync_all()
    });
    match written.and_then(|()| fs::rename(&partial, &request.out)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(VoiceError::disk(e))
        }
    }
}

fn partial_path(out: &Path) -> PathBuf {
    let mut name = out.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    out.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    impl Render for Fake {
        fn render(&self, line: &Line) -> Result<Clip, VoiceError> {
            if line.question == "fail" {
                Err(VoiceError::Render)
            } else {
                Ok(Clip { samples: vec![0.5; line.question.len()], rate: 8000 })
            }
        }
    }

    #[test]
    fn each_request_is_answered_in_order() {
        let dir = std::env::temp_dir().join(format!("voice-server-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let request = |text: &str, file: &str| {
            let line = Line { question: text.into(), answers: vec![], language: quiz_core::QuizLanguage::Cs, voice: 0, speed: 100 };
            serde_json::to_string(&Request { line, out: dir.join(file) }).unwrap()
        };
        let input = [request("ahoj", "a.wav"), "not json".into(), request("fail", "b.wav"), request("svete", "c.wav")].join("\n");
        let mut output = Vec::new();
        serve(&Fake, input.as_bytes(), &mut output).unwrap();
        let replies: Vec<Reply> = String::from_utf8(output).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
        assert_eq!(replies, [Reply::Ready, Reply::Written, Reply::Failed, Reply::Failed, Reply::Written]);
        assert_eq!(fs::metadata(dir.join("a.wav")).unwrap().len(), 44 + 8);
        assert!(!dir.join("b.wav").exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "no partial file is left");
        let _ = fs::remove_dir_all(&dir);
    }
}
