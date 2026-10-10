//! What the Director's media player plays (spec SN-7): one file of the media
//! library at a time, started from the list, the toolbar or a pad of the
//! sound board. The Director plays sounds; the projector shows pictures and
//! videos over the stage. It is no show state, so it is not journaled.

use crate::media::MediaKind;
use crate::sound_board::{BoardError, PadClick, PadSource, SoundBoard};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The file that plays, or stands paused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    /// Tells one start from the next, so the end of an earlier one stops nothing.
    pub id: u64,
    pub reference: String,
    pub kind: MediaKind,
    /// Where it was when it last started or paused.
    pub position_ms: u64,
    /// When it last started from `position_ms`; none while paused.
    pub since_ms: Option<u64>,
}

impl NowPlaying {
    pub fn playing(&self) -> bool {
        self.since_ms.is_some()
    }

    /// Where it is at `at_ms`.
    pub fn position_at(&self, at_ms: u64) -> u64 {
        self.position_ms + self.since_ms.map_or(0, |since| at_ms.saturating_sub(since))
    }

    fn pause(&mut self, at_ms: u64) {
        self.position_ms = self.position_at(at_ms);
        self.since_ms = None;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerCommand {
    /// Plays a library file from its start, or on where it stands paused.
    Play { reference: String },
    /// Plays the file on pad `pad`, or, when it plays already, does what the
    /// board says a click on a playing pad does. A pad with the slide's
    /// sound or video plays it through the show, so here it does nothing.
    Pad { pad: usize },
    Pause,
    Resume,
    Stop,
    /// Back to the start, playing on if it played.
    Rewind,
    /// The file started as `id` reached its end.
    Ended { id: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaPlayer {
    now: Option<NowPlaying>,
    last_id: u64,
}

impl MediaPlayer {
    pub fn now(&self) -> Option<&NowPlaying> {
        self.now.as_ref()
    }

    /// Carries out `command`, given at `at_ms`, with the files of `library`
    /// and the pads of `board`.
    pub fn apply(&mut self, command: PlayerCommand, board: &SoundBoard, library: &[String], at_ms: u64) -> Result<(), BoardError> {
        match command {
            PlayerCommand::Play { reference } => {
                if !library.contains(&reference) {
                    return Err(BoardError::NotInLibrary(reference));
                }
                match &mut self.now {
                    Some(now) if now.reference == reference && !now.playing() => now.since_ms = Some(at_ms),
                    _ => self.start(reference, at_ms),
                }
            }
            PlayerCommand::Pad { pad } => {
                let Some(PadSource::File { reference }) = board.pad(pad)?.source.clone() else { return Ok(()) };
                match &mut self.now {
                    Some(now) if now.reference == reference && now.playing() => match board.click {
                        PadClick::Stop => self.now = None,
                        PadClick::Restart => self.start(reference, at_ms),
                        PadClick::Pause => now.pause(at_ms),
                    },
                    Some(now) if now.reference == reference => now.since_ms = Some(at_ms),
                    _ => self.start(reference, at_ms),
                }
            }
            PlayerCommand::Pause => {
                if let Some(now) = &mut self.now {
                    now.pause(at_ms);
                }
            }
            PlayerCommand::Resume => {
                if let Some(now) = self.now.as_mut().filter(|now| !now.playing()) {
                    now.since_ms = Some(at_ms);
                }
            }
            PlayerCommand::Stop => self.now = None,
            PlayerCommand::Rewind => {
                if let Some(now) = &mut self.now {
                    now.position_ms = 0;
                    now.since_ms = now.since_ms.map(|_| at_ms);
                }
            }
            PlayerCommand::Ended { id } => {
                if self.now.as_ref().is_some_and(|now| now.id == id) {
                    self.now = None;
                }
            }
        }
        Ok(())
    }

    /// The media library is now `library`: a file that left it stops.
    pub fn library_changed(&mut self, library: &[String]) {
        if self.now.as_ref().is_some_and(|now| !library.contains(&now.reference)) {
            self.now = None;
        }
    }

    fn start(&mut self, reference: String, at_ms: u64) {
        self.last_id += 1;
        // The library holds only files of a known kind; a stray one counts as a sound, which shows nothing.
        let kind = MediaKind::of(&reference).unwrap_or(MediaKind::Audio);
        self.now = Some(NowPlaying { id: self.last_id, reference, kind, position_ms: 0, since_ms: Some(at_ms) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_board::BoardChange;

    const SONG: &str = "media/song.mp3";
    const CLIP: &str = "media/clip.mp4";

    fn library() -> Vec<String> {
        vec![SONG.to_string(), CLIP.to_string(), "media/flag.png".to_string()]
    }

    /// A board that filled itself with the library: the song on pad 0, the clip on pad 1.
    fn board(click: PadClick) -> SoundBoard {
        let mut board = SoundBoard::default().normalized(&library());
        board.click = click;
        board
    }

    fn run(player: &mut MediaPlayer, board: &SoundBoard, command: PlayerCommand, at_ms: u64) {
        player.apply(command, board, &library(), at_ms).unwrap();
    }

    fn play(reference: &str) -> PlayerCommand {
        PlayerCommand::Play { reference: reference.into() }
    }

    #[test]
    fn a_file_plays_pauses_and_goes_on_where_it_stood() {
        let board = board(PadClick::Stop);
        let mut player = MediaPlayer::default();
        run(&mut player, &board, play(SONG), 1_000);
        let now = player.now().unwrap().clone();
        assert_eq!((now.kind, now.position_at(3_500), now.playing()), (MediaKind::Audio, 2_500, true));
        run(&mut player, &board, PlayerCommand::Pause, 3_500);
        assert_eq!(player.now().unwrap().position_at(9_000), 2_500, "paused, it stands still");
        run(&mut player, &board, play(SONG), 10_000);
        assert_eq!(player.now().unwrap().position_at(10_500), 3_000, "play goes on from the pause");
        assert_eq!(player.now().unwrap().id, now.id);
        run(&mut player, &board, PlayerCommand::Rewind, 11_000);
        assert_eq!(player.now().unwrap().position_at(11_200), 200);
        run(&mut player, &board, play(CLIP), 12_000);
        assert_eq!((player.now().unwrap().kind, player.now().unwrap().position_at(12_000)), (MediaKind::Video, 0));
        run(&mut player, &board, PlayerCommand::Stop, 13_000);
        assert_eq!(player.now(), None);
    }

    #[test]
    fn a_click_on_the_playing_pad_does_what_the_board_says() {
        let expect = |click: PadClick| {
            let board = board(click);
            let mut player = MediaPlayer::default();
            run(&mut player, &board, PlayerCommand::Pad { pad: 0 }, 1_000);
            run(&mut player, &board, PlayerCommand::Pad { pad: 0 }, 2_000);
            player.now().map(|now| (now.position_at(3_000), now.playing()))
        };
        assert_eq!(expect(PadClick::Stop), None);
        assert_eq!(expect(PadClick::Restart), Some((1_000, true)));
        assert_eq!(expect(PadClick::Pause), Some((1_000, false)));

        // A paused pad goes on, whatever the board says.
        let board = board(PadClick::Stop);
        let mut player = MediaPlayer::default();
        run(&mut player, &board, PlayerCommand::Pad { pad: 0 }, 1_000);
        run(&mut player, &board, PlayerCommand::Pause, 2_000);
        run(&mut player, &board, PlayerCommand::Pad { pad: 0 }, 5_000);
        assert_eq!(player.now().map(|now| now.position_at(5_500)), Some(1_500));
        // Another pad starts its own file.
        run(&mut player, &board, PlayerCommand::Pad { pad: 1 }, 6_000);
        assert_eq!(player.now().map(|now| now.reference.as_str()), Some(CLIP));
    }

    #[test]
    fn pads_without_a_file_play_nothing_here() {
        let mut board = board(PadClick::Stop);
        board.apply(BoardChange::SetSource { pad: 2, source: Some(PadSource::SlideSound) }, &library()).unwrap();
        let mut player = MediaPlayer::default();
        run(&mut player, &board, PlayerCommand::Pad { pad: 2 }, 1_000);
        run(&mut player, &board, PlayerCommand::Pad { pad: 3 }, 1_000);
        assert_eq!(player.now(), None);
        assert_eq!(player.apply(PlayerCommand::Pad { pad: 4 }, &board, &library(), 1_000), Err(BoardError::NoSuchPad(4)));
        assert_eq!(player.apply(play("media/x.mp3"), &board, &library(), 1_000), Err(BoardError::NotInLibrary("media/x.mp3".into())));
    }

    #[test]
    fn only_the_end_of_what_plays_now_stops_it() {
        let board = board(PadClick::Stop);
        let mut player = MediaPlayer::default();
        run(&mut player, &board, play(SONG), 1_000);
        let first = player.now().unwrap().id;
        run(&mut player, &board, PlayerCommand::Pad { pad: 1 }, 2_000);
        run(&mut player, &board, PlayerCommand::Ended { id: first }, 3_000);
        assert_eq!(player.now().map(|now| now.reference.as_str()), Some(CLIP), "the song's end came after the clip started");
        let second = player.now().unwrap().id;
        run(&mut player, &board, PlayerCommand::Ended { id: second }, 4_000);
        assert_eq!(player.now(), None);
    }

    #[test]
    fn a_file_that_leaves_the_library_stops() {
        let board = board(PadClick::Stop);
        let mut player = MediaPlayer::default();
        run(&mut player, &board, play(SONG), 1_000);
        player.library_changed(&[CLIP.to_string(), SONG.to_string()]);
        assert!(player.now().is_some());
        player.library_changed(&[CLIP.to_string()]);
        assert_eq!(player.now(), None);
    }
}
