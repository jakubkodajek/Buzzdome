//! Quiz model, live game state machine and scoring. No UI and no I/O, so the
//! rules can be tested in isolation and replayed from a journal.

pub mod game;
pub mod media;
pub mod model;
pub mod view;

pub use game::{BuzzerId, Button, Clock, Command, Cue, Effect, Game, GameError, Light, Phase, QuestionStep, Stamped};
pub use media::{is_safe_reference, MediaKind, SoundClip, SoundStart, VideoClip, MEDIA_PREFIX};
pub use model::{AnswerMode, Question, Quiz, Slide, SlideContent, ValidationError, SCHEMA_VERSION};
pub use view::GameView;
