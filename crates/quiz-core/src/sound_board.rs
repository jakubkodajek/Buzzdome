//! The Director's sound board (spec SN-7): a grid of pads, each playing a
//! file of the media library, or the sound or video of the slide on screen,
//! with one click. It is no show state: the app keeps it per computer, next
//! to the media library.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MIN_ROWS: u8 = 3;
pub const MAX_ROWS: u8 = 8;
pub const MIN_COLUMNS: u8 = 4;
pub const MAX_COLUMNS: u8 = 8;
/// Places on the largest grid; the pads keep theirs when the grid changes size.
const PLACES: usize = MAX_ROWS as usize * MAX_COLUMNS as usize;
/// Longest pad label, in characters.
pub const MAX_PAD_LABEL_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct SoundBoard {
    pub rows: u8,
    pub columns: u8,
    /// Every place of the largest grid, row by row with `MAX_COLUMNS` places
    /// each; the grid shows its first `rows` rows and `columns` columns.
    pub pads: Vec<Pad>,
    pub click: PadClick,
    /// The quizmaster arranged the pads, so new media files no longer go on
    /// them by themselves.
    pub arranged: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Pad {
    #[serde(default)]
    pub source: Option<PadSource>,
    /// Shown instead of the file name; empty shows the name.
    #[serde(default)]
    pub label: String,
    /// None colours the pad by its column.
    #[serde(default)]
    pub color: Option<PadColor>,
}

/// What a pad plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PadSource {
    /// A file of the media library (`media/<name>`).
    File { reference: String },
    /// The sound of the slide on screen.
    SlideSound,
    /// The video of the slide on screen.
    SlideVideo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PadColor {
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
}

/// What a click on the pad that is playing does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PadClick {
    #[default]
    Stop,
    Restart,
    Pause,
}

/// A change the quizmaster makes to the board. Pads are numbered by their
/// place (see `SoundBoard::pads`).
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BoardChange {
    /// Puts library files on the pad `pad` and the next empty pads shown. A
    /// board that fills itself holds each file once, so there they move.
    #[serde(rename_all = "camelCase")]
    Place { pad: usize, references: Vec<String> },
    /// Sets what the pad plays; none clears it.
    #[serde(rename_all = "camelCase")]
    SetSource { pad: usize, source: Option<PadSource> },
    #[serde(rename_all = "camelCase")]
    SetLabel { pad: usize, label: String },
    #[serde(rename_all = "camelCase")]
    SetColor { pad: usize, color: Option<PadColor> },
    /// Empties the pad, its label and colour too.
    #[serde(rename_all = "camelCase")]
    ClearPad { pad: usize },
    #[serde(rename_all = "camelCase")]
    Swap { a: usize, b: usize },
    #[serde(rename_all = "camelCase")]
    Resize { rows: u8, columns: u8 },
    #[serde(rename_all = "camelCase")]
    SetClick { click: PadClick },
    /// Puts the library files that are on no pad on the empty pads shown.
    FillEmpty,
    ClearAll,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BoardError {
    #[error("there is no pad {0} on the board")]
    NoSuchPad(usize),
    #[error("{0} is not in the media library")]
    NotInLibrary(String),
}

impl Default for SoundBoard {
    fn default() -> SoundBoard {
        SoundBoard { rows: MIN_ROWS, columns: MIN_COLUMNS, pads: vec![Pad::default(); PLACES], click: PadClick::Stop, arranged: false }
    }
}

impl SoundBoard {
    /// A stored board as the app uses it, whatever a hand edit did to the
    /// file: the grid within its limits, a place for every pad, labels cut to
    /// length, and only files of `library` (`library_changed`).
    pub fn normalized(mut self, library: &[String]) -> SoundBoard {
        self.rows = self.rows.clamp(MIN_ROWS, MAX_ROWS);
        self.columns = self.columns.clamp(MIN_COLUMNS, MAX_COLUMNS);
        self.pads.resize(PLACES, Pad::default());
        for pad in &mut self.pads {
            pad.label = clean_label(&pad.label);
        }
        self.library_changed(library);
        self
    }

    /// The places the grid shows, row by row.
    pub fn shown(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.rows as usize).flat_map(move |row| (0..self.columns as usize).map(move |column| row * MAX_COLUMNS as usize + column))
    }

    /// The media library is now `library`: a file that left it leaves its
    /// pads, and a board the quizmaster has not arranged takes the new files.
    pub fn library_changed(&mut self, library: &[String]) {
        for pad in &mut self.pads {
            if matches!(&pad.source, Some(PadSource::File { reference }) if !library.contains(reference)) {
                pad.source = None;
            }
        }
        if !self.arranged {
            self.fill_empty(library);
        }
    }

    pub fn apply(&mut self, change: BoardChange, library: &[String]) -> Result<(), BoardError> {
        match change {
            BoardChange::Place { pad, references } => {
                self.check_shown(pad)?;
                if let Some(unknown) = references.iter().find(|reference| !library.contains(reference)) {
                    return Err(BoardError::NotInLibrary(unknown.clone()));
                }
                if !self.arranged {
                    for pad in &mut self.pads {
                        if matches!(&pad.source, Some(PadSource::File { reference }) if references.contains(reference)) {
                            pad.source = None;
                        }
                    }
                }
                let empty_after = self.shown().skip_while(|&place| place != pad).skip(1).filter(|&place| self.pads[place].source.is_none());
                let places: Vec<usize> = std::iter::once(pad).chain(empty_after).collect();
                for (place, reference) in places.into_iter().zip(references) {
                    self.pads[place].source = Some(PadSource::File { reference });
                }
            }
            BoardChange::SetSource { pad, source } => {
                self.check_shown(pad)?;
                if let Some(PadSource::File { reference }) = &source {
                    if !library.contains(reference) {
                        return Err(BoardError::NotInLibrary(reference.clone()));
                    }
                }
                self.pads[pad].source = source;
            }
            BoardChange::SetLabel { pad, label } => {
                self.check_shown(pad)?;
                self.pads[pad].label = clean_label(&label);
            }
            BoardChange::SetColor { pad, color } => {
                self.check_shown(pad)?;
                self.pads[pad].color = color;
            }
            BoardChange::ClearPad { pad } => {
                self.check_shown(pad)?;
                self.pads[pad] = Pad::default();
            }
            BoardChange::Swap { a, b } => {
                self.check_shown(a)?;
                self.check_shown(b)?;
                self.pads.swap(a, b);
            }
            BoardChange::Resize { rows, columns } => {
                self.rows = rows.clamp(MIN_ROWS, MAX_ROWS);
                self.columns = columns.clamp(MIN_COLUMNS, MAX_COLUMNS);
                // A board left to fill itself fills the pads it now shows too.
                if !self.arranged {
                    self.fill_empty(library);
                }
                return Ok(());
            }
            BoardChange::SetClick { click } => {
                self.click = click;
                return Ok(());
            }
            BoardChange::FillEmpty => self.fill_empty(library),
            BoardChange::ClearAll => self.pads.iter_mut().for_each(|pad| *pad = Pad::default()),
        }
        self.arranged = true;
        Ok(())
    }

    /// The pad at place `pad`, if the grid shows it.
    pub fn pad(&self, pad: usize) -> Result<&Pad, BoardError> {
        self.check_shown(pad)?;
        Ok(&self.pads[pad])
    }

    fn check_shown(&self, pad: usize) -> Result<(), BoardError> {
        if self.shown().any(|place| place == pad) {
            Ok(())
        } else {
            Err(BoardError::NoSuchPad(pad))
        }
    }

    /// Puts the files of `library` that are on no pad on the empty pads shown, in order.
    fn fill_empty(&mut self, library: &[String]) {
        let on_board = |pads: &[Pad], file: &String| pads.iter().any(|pad| matches!(&pad.source, Some(PadSource::File { reference }) if reference == file));
        let mut free = library.iter().filter(|file| !on_board(&self.pads, file)).cloned().collect::<Vec<_>>().into_iter();
        let empty: Vec<usize> = self.shown().filter(|&place| self.pads[place].source.is_none()).collect();
        for place in empty {
            let Some(reference) = free.next() else { break };
            self.pads[place].source = Some(PadSource::File { reference });
        }
    }
}

fn clean_label(label: &str) -> String {
    label.trim().chars().take(MAX_PAD_LABEL_CHARS).collect::<String>().trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| format!("media/{name}")).collect()
    }

    fn file(name: &str) -> Option<PadSource> {
        Some(PadSource::File { reference: format!("media/{name}") })
    }

    /// The files on the pads shown, row by row; "-" for an empty pad.
    fn shown(board: &SoundBoard) -> Vec<String> {
        board
            .shown()
            .map(|place| match &board.pads[place].source {
                Some(PadSource::File { reference }) => reference.trim_start_matches("media/").to_string(),
                Some(PadSource::SlideSound) => "slide sound".into(),
                Some(PadSource::SlideVideo) => "slide video".into(),
                None => "-".into(),
            })
            .collect()
    }

    #[test]
    fn a_new_board_fills_itself_with_the_library_until_the_quizmaster_arranges_it() {
        let mut board = SoundBoard::default().normalized(&library(&["a.mp3", "b.mp4"]));
        assert_eq!(board.shown().count(), 12);
        assert_eq!(shown(&board)[..3], ["a.mp3", "b.mp4", "-"]);
        board.library_changed(&library(&["a.mp3", "b.mp4", "c.png"]));
        assert_eq!(shown(&board)[..3], ["a.mp3", "b.mp4", "c.png"]);

        board.apply(BoardChange::Swap { a: 0, b: 2 }, &library(&["a.mp3", "b.mp4", "c.png"])).unwrap();
        assert!(board.arranged);
        board.library_changed(&library(&["a.mp3", "b.mp4", "c.png", "d.wav"]));
        assert_eq!(shown(&board)[..4], ["c.png", "b.mp4", "a.mp3", "-"], "an arranged board takes no new files by itself");
        board.apply(BoardChange::FillEmpty, &library(&["a.mp3", "b.mp4", "c.png", "d.wav"])).unwrap();
        assert_eq!(shown(&board)[..4], ["c.png", "b.mp4", "a.mp3", "d.wav"]);
    }

    #[test]
    fn a_file_that_leaves_the_library_leaves_its_pads() {
        let all = library(&["a.mp3", "b.mp4"]);
        let mut board = SoundBoard::default().normalized(&all);
        board.apply(BoardChange::SetSource { pad: 9, source: file("a.mp3") }, &all).unwrap();
        board.apply(BoardChange::SetLabel { pad: 9, label: "Potlesk".into() }, &all).unwrap();
        board.library_changed(&library(&["b.mp4"]));
        assert_eq!(shown(&board)[..2], ["-", "b.mp4"]);
        assert_eq!(board.pads[9].source, None);
        assert_eq!(board.pads[9].label, "Potlesk", "the pad keeps its label and colour");
    }

    #[test]
    fn dropped_files_go_on_the_pad_and_the_next_empty_ones() {
        let all = library(&["a.mp3", "b.mp3", "c.mp3", "d.mp3"]);
        let mut board = SoundBoard { arranged: true, ..SoundBoard::default() };
        board.apply(BoardChange::SetSource { pad: 2, source: Some(PadSource::SlideSound) }, &all).unwrap();
        board.apply(BoardChange::Place { pad: 1, references: library(&["a.mp3", "b.mp3", "c.mp3"]) }, &all).unwrap();
        // Past the end of a row, they go on in the next one: place 8 starts row 2.
        assert_eq!(shown(&board)[..6], ["-", "a.mp3", "slide sound", "b.mp3", "c.mp3", "-"]);
        assert_eq!(board.pads[MAX_COLUMNS as usize].source, file("c.mp3"));
        // The pad dropped on is replaced, and full pads are passed over.
        board.apply(BoardChange::Place { pad: 3, references: library(&["d.mp3"]) }, &all).unwrap();
        assert_eq!(shown(&board)[3], "d.mp3");
        board.apply(BoardChange::Place { pad: 3, references: library(&["a.mp3", "b.mp3"]) }, &all).unwrap();
        assert_eq!(shown(&board)[..6], ["-", "a.mp3", "slide sound", "a.mp3", "c.mp3", "b.mp3"]);
    }

    #[test]
    fn files_dropped_on_a_board_that_fills_itself_move_there() {
        let all = library(&["a.mp3", "b.mp3", "c.mp3"]);
        let mut board = SoundBoard::default().normalized(&all);
        board.apply(BoardChange::Place { pad: 2, references: library(&["a.mp3"]) }, &all).unwrap();
        assert_eq!(shown(&board)[..4], ["-", "b.mp3", "a.mp3", "-"], "it replaces the file on that pad");
        // Arranged now, a file can sit on two pads.
        board.apply(BoardChange::Place { pad: 0, references: library(&["a.mp3"]) }, &all).unwrap();
        assert_eq!(shown(&board)[..4], ["a.mp3", "b.mp3", "a.mp3", "-"]);
    }

    #[test]
    fn pads_keep_their_places_when_the_grid_changes_size() {
        let all = library(&["a.mp3"]);
        let mut board = SoundBoard { arranged: true, ..SoundBoard::default() };
        board.apply(BoardChange::Resize { rows: 8, columns: 8 }, &all).unwrap();
        board.apply(BoardChange::SetSource { pad: 63, source: file("a.mp3") }, &all).unwrap();
        board.apply(BoardChange::Resize { rows: 2, columns: 20 }, &all).unwrap();
        assert_eq!((board.rows, board.columns), (MIN_ROWS, MAX_COLUMNS), "sizes stay within the limits");
        assert_eq!(board.apply(BoardChange::SetLabel { pad: 63, label: "x".into() }, &all), Err(BoardError::NoSuchPad(63)));
        board.apply(BoardChange::Resize { rows: 8, columns: 8 }, &all).unwrap();
        assert_eq!(board.pads[63].source, file("a.mp3"));
    }

    #[test]
    fn changes_name_pads_shown_and_files_of_the_library() {
        let all = library(&["a.mp3"]);
        let mut board = SoundBoard::default();
        let before = board.clone();
        assert_eq!(board.apply(BoardChange::Swap { a: 0, b: 4 }, &all), Err(BoardError::NoSuchPad(4)), "place 4 is in a column not shown");
        assert_eq!(board.apply(BoardChange::SetSource { pad: 0, source: file("x.mp3") }, &all), Err(BoardError::NotInLibrary("media/x.mp3".into())));
        assert_eq!(board.apply(BoardChange::Place { pad: 0, references: library(&["a.mp3", "x.mp3"]) }, &all), Err(BoardError::NotInLibrary("media/x.mp3".into())));
        assert_eq!(board, before, "a refused change changes nothing");
        board.apply(BoardChange::SetLabel { pad: 0, label: format!("  {}  ", "y".repeat(50)) }, &all).unwrap();
        assert_eq!(board.pads[0].label, "y".repeat(MAX_PAD_LABEL_CHARS));
        board.apply(BoardChange::SetColor { pad: 0, color: Some(PadColor::Teal) }, &all).unwrap();
        board.apply(BoardChange::ClearPad { pad: 0 }, &all).unwrap();
        assert_eq!(board.pads[0], Pad::default());
    }

    #[test]
    fn a_stored_board_is_mended_to_what_the_app_uses() {
        let json = r#"{"rows": 99, "columns": 1, "pads": [{"source": {"type": "file", "reference": "media/gone.mp3"}, "label": "Old"}],
                       "click": "pause", "arranged": true}"#;
        let board = serde_json::from_str::<SoundBoard>(json).unwrap().normalized(&library(&["a.mp3"]));
        assert_eq!((board.rows, board.columns, board.pads.len(), board.click), (MAX_ROWS, MIN_COLUMNS, PLACES, PadClick::Pause));
        assert_eq!(board.pads[0], Pad { source: None, label: "Old".into(), color: None });
        assert!(shown(&board).iter().all(|pad| pad == "-"), "an arranged board is not filled");
    }
}
