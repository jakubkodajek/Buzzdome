//! The colour each light (`quiz_core::Light`) shows on the buzzers, as the
//! quizmaster picks it in the Receiver tab, and what an RGB buzzer shows of
//! it (`Shade`). The colours are the app's, for every quiz; a protocol with
//! lights in colours takes them (`Protocol::set_colours`).

use quiz_core::Light;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use ts_rs::TS;

/// A colour, written `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, TS)]
#[ts(export, type = "string")]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Rgb {
    pub const BLACK: Rgb = Rgb::new(0, 0, 0);

    pub const fn new(red: u8, green: u8, blue: u8) -> Rgb {
        Rgb { red, green, blue }
    }

    /// The colour `text` writes as `#rrggbb`, in either case.
    pub fn parse(text: &str) -> Option<Rgb> {
        let hex = text.strip_prefix('#').filter(|hex| hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()))?;
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
        Some(Rgb::new(channel(0)?, channel(2)?, channel(4)?))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.red, self.green, self.blue)
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Rgb, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgb::parse(&text).ok_or_else(|| de::Error::custom(format!("{text:?} is no #rrggbb colour")))
    }
}

/// The highest level of a buzzer's red, green or blue (`Shade`).
const TOP_LEVEL: u8 = 3;
/// The lowest channel, out of 255, that a shade at high brightness has.
const BRIGHT_FROM: u8 = 128;

/// What an RGB buzzer shows: red, green and blue each at a level from 0
/// (off) to `TOP_LEVEL`, at low or high brightness (the Enjoy 303S,
/// `enjoy303s`). Taken here as low brightness halving each level, so
/// `nearest` maps a dark colour to low brightness (unverified on the
/// buzzers, `docs/hardware-test.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shade {
    pub bright: bool,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Shade {
    /// The shade nearest `colour`: at high brightness when a channel is at
    /// least half on, each channel at the nearest of four steps from 0 to
    /// 255; else at low brightness, each at the nearest of four steps from
    /// 0 to 127.
    pub fn nearest(colour: Rgb) -> Shade {
        let bright = colour.red.max(colour.green).max(colour.blue) >= BRIGHT_FROM;
        // Steps of 255 / 3 at high brightness, and half as long at low.
        let per_255 = if bright { u16::from(TOP_LEVEL) } else { 2 * u16::from(TOP_LEVEL) };
        let level = |value: u8| ((u16::from(value) * per_255 + 127) / 255).min(u16::from(TOP_LEVEL)) as u8;
        Shade { bright, red: level(colour.red), green: level(colour.green), blue: level(colour.blue) }
    }

    /// The colour the shade stands for on screen, which `nearest` gives
    /// the shade back for.
    pub fn rgb(self) -> Rgb {
        let full = if self.bright { 255 } else { 127 };
        let value = |level: u8| (u16::from(level.min(TOP_LEVEL)) * full / u16::from(TOP_LEVEL)) as u8;
        Rgb::new(value(self.red), value(self.green), value(self.blue))
    }

    /// Whether the shade is dark: no red, green or blue at all.
    pub fn is_dark(self) -> bool {
        self.red == 0 && self.green == 0 && self.blue == 0
    }

    /// Whether the two look alike, brightness aside: a buzzer may report
    /// the brightness it showed in its own way (`enjoy303s`).
    pub fn looks_like(self, other: Shade) -> bool {
        (self.red, self.green, self.blue) == (other.red, other.green, other.blue)
    }
}

/// The colour of each light the quizmaster picks. The defaults are those
/// the buzzers always showed: dark off, blue ready, dim white for out of
/// the question or answered, white first, green right, red wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct LightColours {
    /// Out of a question step: between questions, or a result the player has no part in.
    pub off: Rgb,
    pub ready: Rgb,
    /// Answered an everyone-answers question, waiting for its reveal.
    pub answered: Rgb,
    /// Out of the question: buzzed wrong, another player buzzed, or switched off.
    pub locked: Rgb,
    /// Buzzed first, being judged.
    pub first: Rgb,
    pub correct: Rgb,
    pub wrong: Rgb,
}

/// Dim white: the red of a wrong answer next to it is told apart from it.
const DIM_WHITE: Rgb = Rgb::new(0x2a, 0x2a, 0x2a);

impl Default for LightColours {
    fn default() -> Self {
        LightColours {
            off: Rgb::BLACK,
            ready: Rgb::new(0, 0, 0xff),
            answered: DIM_WHITE,
            locked: DIM_WHITE,
            first: Rgb::new(0xff, 0xff, 0xff),
            correct: Rgb::new(0, 0xff, 0),
            wrong: Rgb::new(0xff, 0, 0),
        }
    }
}

/// Why colours cannot be used: the app counts a fastest-finger press only
/// from a buzzer that showed ready, by the colour it reports with the
/// press (`enjoy303s`), so ready must look like no other light, nor dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ColourClash {
    #[error("ready is dark on the buzzers")]
    ReadyDark,
    #[error("ready looks like {light:?} on the buzzers")]
    ReadyLike { light: Light },
}

impl LightColours {
    pub fn of(&self, light: Light) -> Rgb {
        match light {
            Light::Off => self.off,
            Light::Ready => self.ready,
            Light::Answered => self.answered,
            Light::Locked => self.locked,
            Light::First => self.first,
            Light::Correct => self.correct,
            Light::Wrong => self.wrong,
        }
    }

    /// The shade a buzzer shows for `light`.
    pub fn shade(&self, light: Light) -> Shade {
        Shade::nearest(self.of(light))
    }

    /// The colours as the buzzers show them (`Shade::rgb`), for the Receiver tab.
    pub fn shown(&self) -> LightColours {
        let shown = |light| self.shade(light).rgb();
        LightColours {
            off: shown(Light::Off),
            ready: shown(Light::Ready),
            answered: shown(Light::Answered),
            locked: shown(Light::Locked),
            first: shown(Light::First),
            correct: shown(Light::Correct),
            wrong: shown(Light::Wrong),
        }
    }

    /// Whether a buzzer pressed while showing `shown` was not ready: it
    /// showed another light, brightness aside, or was dark, as one never
    /// told anything is.
    pub fn not_ready(&self, shown: Shade) -> bool {
        shown.is_dark() || Light::ALL.into_iter().any(|light| light != Light::Ready && self.shade(light).looks_like(shown))
    }

    /// Whether the buzzers tell ready apart from every other light.
    pub fn check(&self) -> Result<(), ColourClash> {
        let ready = self.shade(Light::Ready);
        if ready.is_dark() {
            return Err(ColourClash::ReadyDark);
        }
        match Light::ALL.into_iter().find(|&light| light != Light::Ready && self.shade(light).looks_like(ready)) {
            Some(light) => Err(ColourClash::ReadyLike { light }),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shade at its brightness, levels 0 to 3.
    fn shades(bright: bool) -> impl Iterator<Item = Shade> {
        (0..64u8).map(move |i| Shade { bright, red: i >> 4, green: (i >> 2) & 3, blue: i & 3 })
    }

    #[test]
    fn colours_read_and_write_as_hex() {
        assert_eq!(Rgb::parse("#2A2a00"), Some(Rgb::new(0x2a, 0x2a, 0)));
        assert_eq!(Rgb::new(0, 0x0f, 0xff).to_string(), "#000fff");
        for text in ["2a2a2a", "#2a2a2", "#2a2a2a00", "#2a2a2g", "#+12345", "#ééé", ""] {
            assert_eq!(Rgb::parse(text), None, "{text}");
        }
        let colours: LightColours = serde_json::from_str(r##"{"answered": "#000000"}"##).unwrap();
        assert_eq!(colours, LightColours { answered: Rgb::BLACK, ..LightColours::default() });
        assert!(serde_json::from_str::<LightColours>(r#"{"ready": "blue"}"#).is_err());
        assert_eq!(serde_json::to_value(LightColours::default()).unwrap()["ready"], "#0000ff");
    }

    #[test]
    fn a_colour_takes_the_nearest_shade() {
        let shade = |bright, red, green, blue| Shade { bright, red, green, blue };
        assert_eq!(Shade::nearest(Rgb::new(255, 128, 0)), shade(true, 3, 2, 0));
        assert_eq!(Shade::nearest(Rgb::new(255, 42, 43)), shade(true, 3, 0, 1));
        // Below half on, at low brightness: each level half as bright.
        assert_eq!(Shade::nearest(Rgb::new(127, 64, 21)), shade(false, 3, 2, 0));
        assert_eq!(Shade::nearest(Rgb::new(0x2a, 0x2a, 0x2a)), shade(false, 1, 1, 1));
        assert_eq!(Shade::nearest(Rgb::new(5, 5, 5)), shade(false, 0, 0, 0));
        assert!(Shade::nearest(Rgb::new(20, 20, 20)).is_dark());
    }

    #[test]
    fn a_shade_stands_for_a_colour_that_gives_it_back() {
        let given: Vec<Shade> = (0..=255u8).step_by(5).flat_map(|v| [Rgb::new(v, 0, v / 2), Rgb::new(v, v, v), Rgb::new(0, v / 3, v)]).map(Shade::nearest).collect();
        // Only a shade at high brightness with a channel above level 1 is given, else low brightness looks the same.
        for shade in shades(false).chain(shades(true).filter(|s| s.red.max(s.green).max(s.blue) > 1)) {
            assert_eq!(Shade::nearest(shade.rgb()), shade, "{:?}", shade.rgb());
        }
        assert!(given.iter().all(|s| !s.bright || s.red.max(s.green).max(s.blue) > 1));
    }

    #[test]
    fn the_default_colours_tell_ready_apart() {
        let colours = LightColours::default();
        assert_eq!(colours.check(), Ok(()));
        assert!(!colours.not_ready(colours.shade(Light::Ready)));
        for light in Light::ALL.into_iter().filter(|&l| l != Light::Ready) {
            assert!(colours.not_ready(colours.shade(light)), "{light:?}");
        }
    }

    #[test]
    fn ready_must_look_like_no_other_light_nor_dark() {
        let ready_like = |light| ColourClash::ReadyLike { light };
        let with = |change: fn(&mut LightColours)| {
            let mut colours = LightColours::default();
            change(&mut colours);
            colours.check()
        };
        assert_eq!(with(|c| c.ready = Rgb::new(10, 10, 10)), Err(ColourClash::ReadyDark));
        assert_eq!(with(|c| c.answered = Rgb::new(0, 0, 0xee)), Err(ready_like(Light::Answered)));
        // Brightness aside: dim blue looks like ready blue.
        assert_eq!(with(|c| c.wrong = Rgb::new(0, 0, 0x7f)), Err(ready_like(Light::Wrong)));
        assert_eq!(with(|c| c.off = Rgb::new(0, 0, 0xff)), Err(ready_like(Light::Off)));
        // Black for answered and any other light is fine.
        assert_eq!(with(|c| c.answered = Rgb::BLACK), Ok(()));
        assert_eq!(with(|c| c.locked = Rgb::new(0x80, 0, 0x80)), Ok(()));
        // A press from a buzzer showing a colour no light has counts.
        assert!(!LightColours::default().not_ready(Shade::nearest(Rgb::new(255, 255, 0))));
    }
}
