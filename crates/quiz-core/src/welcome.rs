//! The welcome screen a quiz's show can open on (spec SF-3), before the
//! buzzers register: a heading by the quiz's logo, an intro picture or
//! video, and a footer, over the theme's welcome background, while the welcome
//! music plays (`SoundSlot::LobbyMusic`). Next goes on to the registration.

use crate::media::{check_reference, MediaIssue, MediaKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// What the logo can be.
const LOGO_KINDS: &[MediaKind] = &[MediaKind::Image];
/// What the intro can be.
const INTRO_KINDS: &[MediaKind] = &[MediaKind::Image, MediaKind::Video];

/// The quiz's welcome screen. Off by default, so the show opens on the
/// registration, as it did before there was one; what it shows is kept
/// while it is off. Each value that does not read reads as its default, so
/// a quiz always opens and plays.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct Welcome {
    /// The show opens on the welcome screen.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub on: bool,
    /// The text on top, by the logo; the quiz's title when empty.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "String")]
    #[ts(as = "String")]
    pub heading: String,
    /// A picture by the heading, `media/<name>`.
    #[serde(with = "crate::style::lenient")]
    #[schemars(with = "Option<String>", pattern(crate::media::MEDIA_PATTERN))]
    #[ts(as = "Option<String>")]
    pub logo: Option<String>,
    /// A picture, or a video that plays over and over without its sound,
    /// in the middle of the screen, `media/<name>`.
    #[serde(with = "crate::style::lenient")]
    #[schemars(with = "Option<String>", pattern(crate::media::MEDIA_PATTERN))]
    #[ts(as = "Option<String>")]
    pub intro: Option<String>,
    /// A text on a strip at the bottom; no strip when empty.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "String")]
    #[ts(as = "String")]
    pub footer: String,
}

/// A file of the welcome screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum WelcomeFile {
    Logo,
    Intro,
}

impl Welcome {
    pub fn is_default(&self) -> bool {
        *self == Welcome::default()
    }

    /// The quiz's files the welcome screen shows, each with its place.
    pub fn files(&self) -> impl Iterator<Item = (WelcomeFile, &str)> {
        let logo = self.logo.as_deref().map(|file| (WelcomeFile::Logo, file));
        logo.into_iter().chain(self.intro.as_deref().map(|file| (WelcomeFile::Intro, file)))
    }

    /// Points the files in `renamed` (old reference to new one) to their new names.
    pub fn rename_media(&mut self, renamed: &BTreeMap<String, String>) {
        for file in [&mut self.logo, &mut self.intro].into_iter().flatten() {
            if let Some(new) = renamed.get(file.as_str()) {
                file.clone_from(new);
            }
        }
    }

    /// Each file that is not a picture (or for the intro, a video) in the
    /// quiz's media folder, with its place.
    pub fn problems(&self) -> impl Iterator<Item = (WelcomeFile, &str, MediaIssue)> {
        self.files().filter_map(|(place, file)| {
            let kinds = match place {
                WelcomeFile::Logo => LOGO_KINDS,
                WelcomeFile::Intro => INTRO_KINDS,
            };
            check_reference(file, kinds).err().map(|issue| (place, file, issue))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Problem, Quiz, ValidationError};

    #[test]
    fn a_welcome_screen_reads_each_value_on_its_own_and_is_left_out_while_it_is_the_default() {
        let read = |json: &str| serde_json::from_str::<Welcome>(json).unwrap();
        let welcome = read(r#"{ "on": true, "heading": "Tonight", "logo": "media/logo.png", "intro": "media/intro.mp4", "footer": "Free drinks" }"#);
        assert_eq!(
            welcome,
            Welcome { on: true, heading: "Tonight".into(), logo: Some("media/logo.png".into()), intro: Some("media/intro.mp4".into()), footer: "Free drinks".into() }
        );
        // A value a build cannot read reads as its default, and the rest stays.
        let odd = read(r#"{ "on": "yes", "heading": 7, "logo": ["media/logo.png"], "intro": "media/hall.jpg", "footer": null, "slideshow": "media/photos" }"#);
        assert_eq!(odd, Welcome { intro: Some("media/hall.jpg".into()), ..Welcome::default() });

        let mut quiz = Quiz::new("Quiz", Vec::new());
        assert!(serde_json::to_value(&quiz).unwrap().get("welcome").is_none());
        quiz.welcome.on = true;
        let json = serde_json::to_string(&quiz).unwrap();
        assert_eq!(Quiz::from_json(&json).unwrap().welcome, quiz.welcome);
        // Not an object at all: no welcome screen, and the quiz still opens.
        let broken = json.replace(r#""welcome":{"#, r#""welcome":"on","unread":{"#);
        assert_eq!(Quiz::from_json(&broken).unwrap().welcome, Welcome::default());
    }

    #[test]
    fn the_welcome_files_are_the_quizs_media_checked_and_renamed_with_it() {
        let mut quiz = Quiz::new("Quiz", Vec::new());
        quiz.welcome.logo = Some("media/logo.png".into());
        quiz.welcome.intro = Some("media/intro.mp4".into());
        assert!(quiz.validate().is_ok());
        assert_eq!(quiz.media_files().into_iter().collect::<Vec<_>>(), ["media/intro.mp4", "media/logo.png"]);

        let renamed = BTreeMap::from([("media/logo.png".to_string(), "media/logo-2.png".to_string())]);
        quiz.rename_media(&renamed);
        assert_eq!(quiz.welcome.logo.as_deref(), Some("media/logo-2.png"));

        // A logo is a picture; the intro may be a video, never a sound.
        quiz.welcome.logo = Some("media/logo.mp4".into());
        quiz.welcome.intro = Some("../intro.mp3".into());
        let problems = quiz.validate().unwrap_err();
        let place = |problem: &ValidationError| match &problem.problem {
            Problem::WelcomeMedia { place, issue, .. } => Some((*place, issue.clone())),
            _ => None,
        };
        assert_eq!(
            problems.iter().map(place).collect::<Vec<_>>(),
            [
                Some((WelcomeFile::Logo, MediaIssue::WrongType { expected: vec![MediaKind::Image] })),
                Some((WelcomeFile::Intro, MediaIssue::NotInMediaFolder)),
            ]
        );
        assert!(problems.iter().all(|problem| problem.slide_id.is_none() && problem.problem.keeps_from_playing()));
    }
}
