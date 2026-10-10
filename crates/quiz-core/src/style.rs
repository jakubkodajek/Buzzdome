//! How a slide looks apart from its theme: a gradient behind the slide, the
//! colour, font and fill of its texts, and how a question's clock is drawn.
//! Every value is optional and falls back to the theme's. A value a file has
//! wrong (a colour that is not hex, a size out of range) reads as missing, so
//! a hand-edited style never keeps a quiz from opening.

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

/// Smallest text size, in percent of the theme's size for that text.
pub const MIN_TEXT_SIZE: u16 = 50;
/// Largest text size, in percent of the theme's size for that text.
pub const MAX_TEXT_SIZE: u16 = 200;
/// Longest font name, in characters.
pub const MAX_FONT_NAME_CHARS: usize = 64;
/// Most transparency, in percent: the fill no longer shows.
pub const MAX_TRANSPARENCY: u8 = 100;

/// Reads `$name` as a `$inner` it takes through its `TryFrom`, so a value
/// that does not fit does not read.
macro_rules! deserialize_checked {
    ($name:ident, $inner:ty) => {
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<$name, D::Error> {
                $name::try_from(<$inner>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }
    };
}

deserialize_checked!(Color, String);
deserialize_checked!(FontName, String);
deserialize_checked!(TextSize, u16);
deserialize_checked!(Transparency, u8);

/// Whether `color` is `#RRGGBB`, or `#RRGGBBAA` with its opacity.
pub fn is_hex_color(color: &str) -> bool {
    color.strip_prefix('#').is_some_and(|hex| matches!(hex.len(), 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A colour as `#RRGGBB`, or `#RRGGBBAA` with its opacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Color(String);

impl TryFrom<String> for Color {
    type Error = &'static str;

    fn try_from(color: String) -> Result<Color, Self::Error> {
        is_hex_color(&color).then_some(Color(color)).ok_or("a colour is #RRGGBB or #RRGGBBAA")
    }
}

impl Color {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The name of a font installed on the computer that shows the quiz. One
/// that is not installed there falls back to the theme's font.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FontName(String);

impl TryFrom<String> for FontName {
    type Error = &'static str;

    /// Not empty, not too long, and nothing that would end the name in a
    /// style sheet.
    fn try_from(name: String) -> Result<FontName, Self::Error> {
        let fits = !name.trim().is_empty()
            && name.chars().count() <= MAX_FONT_NAME_CHARS
            && !name.chars().any(|c| c.is_control() || "\"'\\;{}<>".contains(c));
        fits.then_some(FontName(name)).ok_or("not a font name")
    }
}

/// A text's size in percent of the theme's size for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct TextSize(u16);

impl TryFrom<u16> for TextSize {
    type Error = &'static str;

    fn try_from(percent: u16) -> Result<TextSize, Self::Error> {
        (MIN_TEXT_SIZE..=MAX_TEXT_SIZE).contains(&percent).then_some(TextSize(percent)).ok_or("text size out of range")
    }
}

/// How much of a fill shows through, in percent: 0 is solid, `MAX_TRANSPARENCY` invisible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Transparency(u8);

impl TryFrom<u8> for Transparency {
    type Error = &'static str;

    fn try_from(percent: u8) -> Result<Transparency, Self::Error> {
        (percent <= MAX_TRANSPARENCY).then_some(Transparency(percent)).ok_or("transparency out of range")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum GradientKind {
    /// From the top down.
    Vertical,
    /// From the left to the right.
    Horizontal,
    /// From the top left corner to the bottom right one.
    Diagonal,
    /// From the bottom left corner to the top right one.
    DiagonalUp,
    /// From the middle out.
    Radial,
}

/// A fill from one colour to another; both the same is a plain colour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Gradient {
    pub from: Color,
    pub to: Color,
    pub kind: GradientKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// How the projector draws a question's clock: the time left and the
/// question's place in the quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TimerDesign {
    /// A dial whose ring empties, the seconds inside (the default).
    Ring,
    /// A bar that empties from right to left, the seconds beside it.
    Bar,
    /// Big lit digits, minutes and seconds, over a thin line that empties.
    Digital,
    /// An hourglass whose sand runs down.
    Hourglass,
    /// A stopwatch whose hand and slice go round as the time runs.
    Stopwatch,
}

/// The look of a text on a slide. What is missing is the theme's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<Color>", optional)]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<FontName>", optional)]
    pub font: Option<FontName>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<TextSize>", optional)]
    pub size: Option<TextSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<bool>", optional)]
    pub bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<bool>", optional)]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<bool>", optional)]
    pub underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<TextAlign>", optional)]
    pub align: Option<TextAlign>,
    /// A drop shadow under the text (a title and a question have one in
    /// every built-in theme).
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<bool>", optional)]
    pub shadow: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<Color>", optional)]
    pub shadow_color: Option<Color>,
    /// Behind the text: an answer's panel, or a box around a title or text.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<Gradient>", optional)]
    pub fill: Option<Gradient>,
    /// Of the fill, the theme's or `fill`.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<Transparency>", optional)]
    pub transparency: Option<Transparency>,
}

impl TextStyle {
    /// Whether the text looks as the theme has it.
    pub fn is_empty(&self) -> bool {
        self == &TextStyle::default()
    }
}

/// A slide's own look on top of its theme.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SlideStyle {
    /// Behind the slide, unless it has a background picture or video.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<Gradient>", optional)]
    pub fill: Option<Gradient>,
    /// The question, or the title of another slide: the headline, whose
    /// look a change of type keeps (an explanation's headline is its text).
    #[serde(default, skip_serializing_if = "TextStyle::is_empty", with = "lenient_or_default")]
    #[ts(as = "TextStyle")]
    pub headline: TextStyle,
    /// A billboard's or an explanation's text.
    #[serde(default, skip_serializing_if = "TextStyle::is_empty", with = "lenient_or_default")]
    #[ts(as = "TextStyle")]
    pub text: TextStyle,
    /// Every answer of a question alike.
    #[serde(default, skip_serializing_if = "TextStyle::is_empty", with = "lenient_or_default")]
    #[ts(as = "TextStyle")]
    pub answers: TextStyle,
    /// A question's clock; kept on other slides, which have none, for a
    /// change of type. None is a ring.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "lenient")]
    #[ts(as = "Option<TimerDesign>", optional)]
    pub timer: Option<TimerDesign>,
}

impl SlideStyle {
    /// Whether the slide looks as its theme has it.
    pub fn is_empty(&self) -> bool {
        self == &SlideStyle::default()
    }

    /// What `written`, this style as a file has it, holds that the style
    /// read without (a value of the wrong type or out of range, or a name
    /// it does not know), each by its path in the file: `style.fill`,
    /// `style.headline.color`. Nulls and empty texts are nothing left out.
    pub fn left_out(&self, written: &serde_json::Value) -> Vec<String> {
        let read = serde_json::to_value(self).expect("a style serializes: strings, numbers and flags");
        let mut paths = Vec::new();
        left_out(written, &read, "style".to_owned(), &mut paths);
        paths
    }
}

/// Adds to `paths` what of `written` (at `path`) is not in `read`, looking
/// into what both have as objects.
fn left_out(written: &serde_json::Value, read: &serde_json::Value, path: String, paths: &mut Vec<String>) {
    use serde_json::Value;
    match (written, read) {
        (Value::Object(written), Value::Object(read)) => {
            for (key, value) in written {
                left_out(value, read.get(key).unwrap_or(&Value::Null), format!("{path}.{key}"), paths);
            }
        }
        _ if written != read && !is_blank(written) => paths.push(path),
        _ => {}
    }
}

/// Whether `value` holds nothing: null, or an object of nothing.
fn is_blank(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::Object(fields) => fields.values().all(is_blank),
        _ => false,
    }
}

/// An optional value as `#[serde(with)]` module: one that does not read
/// (a wrong type, or out of range) reads as none.
pub(crate) mod lenient {
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error> {
        value.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: DeserializeOwned>(deserializer: D) -> Result<Option<T>, D::Error> {
        let value = Option::<serde_json::Value>::deserialize(deserializer)?;
        Ok(value.and_then(|value| serde_json::from_value(value).ok()))
    }
}

/// A value with a default as `#[serde(with)]` module: one that does not read
/// reads as the default.
pub(crate) mod lenient_or_default {
    use serde::de::DeserializeOwned;
    use serde::{Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
        value.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: DeserializeOwned + Default>(deserializer: D) -> Result<T, D::Error> {
        Ok(super::lenient::deserialize(deserializer)?.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(hex: &str) -> Color {
        Color::try_from(hex.to_owned()).unwrap()
    }

    #[test]
    fn a_colour_is_six_or_eight_hex_digits() {
        assert!(is_hex_color("#1E4A6B") && is_hex_color("#1e4a6bcc"));
        for wrong in ["1E4A6B", "#fff", "#1E4A6", "#1E4A6BC", "#GGGGGG", "red", ""] {
            assert!(!is_hex_color(wrong), "{wrong}");
        }
    }

    #[test]
    fn a_style_keeps_only_what_is_set_and_reads_back_the_same() {
        let style = SlideStyle {
            fill: Some(Gradient { from: color("#102030"), to: color("#405060"), kind: GradientKind::DiagonalUp }),
            headline: TextStyle {
                color: Some(color("#FFFFFF")),
                font: Some(FontName::try_from("Segoe UI".to_owned()).unwrap()),
                size: Some(TextSize::try_from(120).unwrap()),
                bold: Some(false),
                align: Some(TextAlign::Left),
                transparency: Some(Transparency::try_from(40).unwrap()),
                ..TextStyle::default()
            },
            ..SlideStyle::default()
        };
        let json = serde_json::to_value(&style).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "fill": { "from": "#102030", "to": "#405060", "kind": "diagonalUp" },
                "headline": { "color": "#FFFFFF", "font": "Segoe UI", "size": 120, "bold": false, "align": "left", "transparency": 40 }
            })
        );
        assert_eq!(serde_json::from_value::<SlideStyle>(json).unwrap(), style);
        assert_eq!(serde_json::to_value(SlideStyle::default()).unwrap(), serde_json::json!({}));
    }

    #[test]
    fn a_timer_design_reads_by_its_name_and_an_unknown_one_is_left_out() {
        let style: SlideStyle = serde_json::from_value(serde_json::json!({ "timer": "hourglass" })).unwrap();
        assert_eq!(style.timer, Some(TimerDesign::Hourglass));
        assert_eq!(serde_json::to_value(&style).unwrap(), serde_json::json!({ "timer": "hourglass" }));
        let written = serde_json::json!({ "timer": "sundial" });
        let style: SlideStyle = serde_json::from_value(written.clone()).unwrap();
        assert_eq!((style.timer, style.left_out(&written)), (None, vec!["style.timer".to_owned()]));
    }

    #[test]
    fn a_wrong_value_reads_as_the_themes_and_leaves_the_rest() {
        let written = serde_json::json!({
            "fill": { "from": "#102030", "to": "blue", "kind": "vertical" },
            "headline": {
                "color": "#fff",
                "font": "x\"; color: red",
                "size": 300,
                "bold": "yes",
                "align": "justify",
                "shadowColor": "#00000080",
                "transparency": 101
            },
            "text": 7,
            "answers": { "italic": true, "fill": { "from": "#000000", "to": "#ffffff", "kind": "conic" } }
        });
        let style: SlideStyle = serde_json::from_value(written.clone()).unwrap();
        assert_eq!(style.fill, None);
        assert_eq!(style.headline, TextStyle { shadow_color: Some(color("#00000080")), ..TextStyle::default() });
        assert!(style.text.is_empty());
        assert_eq!(style.answers, TextStyle { italic: Some(true), ..TextStyle::default() });

        // What it read without is named, so the author can be told.
        let mut left_out = style.left_out(&written);
        left_out.sort();
        let expected = ["answers.fill", "fill", "headline.align", "headline.bold", "headline.color", "headline.font", "headline.size"];
        let expected: Vec<String> = expected.iter().chain(&["headline.transparency", "text"]).map(|path| format!("style.{path}")).collect();
        assert_eq!(left_out, expected);
    }

    #[test]
    fn nothing_is_left_out_of_a_style_that_reads_whole() {
        let written = serde_json::json!({
            "fill": { "from": "#102030", "to": "#405060", "kind": "radial" },
            "headline": { "color": "#FFFFFF", "bold": false, "font": null },
            "text": {},
            "answers": { "size": null }
        });
        let style: SlideStyle = serde_json::from_value(written.clone()).unwrap();
        assert_eq!(style.left_out(&written), Vec::<String>::new());
        assert_eq!(SlideStyle::default().left_out(&serde_json::Value::Null), Vec::<String>::new());
        // A name it does not know, or a style that is not one at all, is left out too.
        let typo = serde_json::json!({ "headline": { "colour": "#FFFFFF" } });
        assert_eq!(serde_json::from_value::<SlideStyle>(typo.clone()).unwrap().left_out(&typo), ["style.headline"]);
        assert_eq!(SlideStyle::default().left_out(&serde_json::json!("dark")), ["style"]);
    }

    #[test]
    fn a_font_name_is_short_and_cannot_end_itself() {
        let name = |s: &str| FontName::try_from(s.to_owned()).is_ok();
        assert!(name("Noto Sans CJK JP") && name("Ubuntu Mono"));
        assert!(name(&"x".repeat(MAX_FONT_NAME_CHARS)));
        assert!(!name(&"x".repeat(MAX_FONT_NAME_CHARS + 1)));
        for wrong in ["", "  ", "A\"B", "A'B", "A;B", "A{B", "A\\B", "A\nB", "<b>"] {
            assert!(!name(wrong), "{wrong:?}");
        }
    }

    #[test]
    fn sizes_and_transparency_stay_in_range() {
        assert!(TextSize::try_from(MIN_TEXT_SIZE).is_ok() && TextSize::try_from(MAX_TEXT_SIZE).is_ok());
        assert!(TextSize::try_from(MIN_TEXT_SIZE - 1).is_err() && TextSize::try_from(MAX_TEXT_SIZE + 1).is_err());
        assert!(Transparency::try_from(MAX_TRANSPARENCY).is_ok() && Transparency::try_from(MAX_TRANSPARENCY + 1).is_err());
    }
}
