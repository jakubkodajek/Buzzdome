//! Turns an Open Fixture Library fixture definition (its JSON format,
//! <https://github.com/OpenLightingProject/open-fixture-library/blob/master/docs/fixture-format.md>,
//! and its capability types, `docs/capability-types.md` there) into our
//! library's modes. Used only by `examples/ofl-import.rs`, when the library
//! is made again; the app never reads this format.
//!
//! Fixtures in the "Laser" category are left out: a show laser is for a
//! trained operator, not for looks a quiz runs on its own (spec LT-9).
//!
//! Every other channel becomes one of ours:
//!
//! - one whose whole range dims (`Intensity`) is a dimmer, the other way
//!   round when it gets darker as its value rises (`DimmerRange`), and one
//!   whose whole range is red, green, blue or white (`ColorIntensity`) that
//!   colour;
//! - a shutter opens while a look lights the fixture and closes otherwise
//!   (`Shutter`, never its strobe range); it needs an open value (an "Open"
//!   range, or a lamp switch's "bright" one) and a dark one (a "Closed"
//!   range, or an `Intensity` range that starts dark);
//! - a dimmer whose top range does something else (strobe, a program)
//!   dims only below it (`DimmerRange`): the app strobes only through a
//!   look's strobe effect, which is limited (`MIN_EFFECT_MS`);
//! - the finer byte of a 16-bit channel is not driven;
//! - anything else (pan, tilt, gobos, programs, fog, maintenance) rests at
//!   its default value, which is what the fixture does when no desk drives
//!   it. When that value strobes, runs a program (an automatic, sound or
//!   chase effect) or lights the fixture, by the definition, the channel
//!   rests at one of its values that does none of these; when it has none,
//!   the mode lists the channel to check on the fixture
//!   ([`LibraryMode::check`]).
//!
//! The manual's name for each channel goes along as its label, so the
//! quizmaster can still set it by hand in a look's extra channels.
//!
//! A definition comes from a download too, so any input gives a result or
//! an error, never a panic or an endless loop: values are shifted with
//! checks, matrices and modes are capped.

use super::{LibraryFixture, LibraryMode};
use crate::setup::{Channel, MAX_FIXTURE_CHANNELS, MAX_LIGHTS_NAME_CHARS};
use crate::UNIVERSE_SIZE;
use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::{Map, Value};
use std::cmp::Ordering;

/// What became of one definition file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Converted {
    Fixture(LibraryFixture),
    /// The file only points to another definition.
    Redirect,
    /// In the "Laser" category: left out.
    Laser,
    /// No mode fits a fixture's channels.
    NoModes,
}

/// `text` (one definition file) of `manufacturer`, at `key`
/// ("manufacturer/model"). Modes with more channels than a fixture may
/// have are left out.
pub fn convert(manufacturer: &str, key: &str, text: &str) -> serde_json::Result<Converted> {
    let fixture: Value = serde_json::from_str(text)?;
    if fixture.get("redirectTo").is_some() {
        return Ok(Converted::Redirect);
    }
    let categories = fixture.get("categories").and_then(Value::as_array);
    if categories.into_iter().flatten().any(|category| category == "Laser") {
        return Ok(Converted::Laser);
    }
    // A JSON value's map forgets the order of its keys, which
    // "eachPixelGroup" follows: read the group keys from the text.
    let groups = serde_json::from_str::<GroupKeys>(text)?.matrix.unwrap_or_default().pixel_groups.0;
    let definition = Definition::new(&fixture, groups);
    let modes: Vec<LibraryMode> = fixture
        .get("modes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mode| {
            let name = fitted(mode.get("name")?.as_str()?);
            let mode = definition.mode(mode, name)?;
            (!mode.channels.is_empty() && mode.channels.len() <= MAX_FIXTURE_CHANNELS.min(UNIVERSE_SIZE)).then_some(mode)
        })
        .collect();
    let name = fixture.get("name").and_then(Value::as_str).map(fitted);
    Ok(match name {
        Some(name) if !modes.is_empty() => Converted::Fixture(LibraryFixture { manufacturer: manufacturer.to_owned(), name, key: key.to_owned(), modes }),
        _ => Converted::NoModes,
    })
}

/// `text` cut to a name's length.
fn fitted(text: &str) -> String {
    text.chars().take(MAX_LIGHTS_NAME_CHARS).collect()
}

/// The part of a definition that holds its pixel groups, in the file's order.
#[derive(serde::Deserialize)]
struct GroupKeys {
    matrix: Option<MatrixGroups>,
}

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatrixGroups {
    #[serde(default)]
    pixel_groups: KeysInOrder,
}

/// An object's keys, in order.
#[derive(Default)]
struct KeysInOrder(Vec<String>);

impl<'de> Deserialize<'de> for KeysInOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Keys;
        impl<'de> Visitor<'de> for Keys {
            type Value = KeysInOrder;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<KeysInOrder, A::Error> {
                let mut keys = Vec::new();
                while let Some((key, IgnoredAny)) = map.next_entry::<String, IgnoredAny>()? {
                    keys.push(key);
                }
                Ok(KeysInOrder(keys))
            }
        }
        deserializer.deserialize_map(Keys)
    }
}

struct Definition<'a> {
    channels: Map<String, Value>,
    templates: Map<String, Value>,
    matrix: Matrix,
    /// The fine channels' keys; a template channel's still hold `$pixelKey`.
    fine: Vec<&'a str>,
}

impl<'a> Definition<'a> {
    fn new(fixture: &'a Value, groups: Vec<String>) -> Definition<'a> {
        let object = |name: &str| fixture.get(name).and_then(Value::as_object).cloned().unwrap_or_default();
        let fine = ["availableChannels", "templateChannels"]
            .into_iter()
            .filter_map(|name| fixture.get(name)?.as_object())
            .flat_map(|channels| channels.values())
            .filter_map(|channel| channel.get("fineChannelAliases")?.as_array())
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let matrix = Matrix::new(fixture.get("matrix"), groups);
        Definition { channels: object("availableChannels"), templates: object("templateChannels"), matrix, fine }
    }

    /// A mode's channels, labels and channels to check; none if a matrix in
    /// it cannot be read.
    fn mode(&self, mode: &Value, name: String) -> Option<LibraryMode> {
        let mut out = LibraryMode { name, channels: Vec::new(), labels: Vec::new(), check: Vec::new() };
        for entry in mode.get("channels")?.as_array()? {
            let keys = match entry {
                Value::String(key) => vec![key.clone()],
                Value::Object(insert) => self.matrix_channels(insert)?,
                // A channel the mode leaves out.
                _ => vec![String::new()],
            };
            if out.channels.len() + keys.len() > UNIVERSE_SIZE {
                return None;
            }
            for key in keys {
                let (channel, check) = self.channel(&key);
                out.channels.push(channel);
                if check {
                    out.check.push(fitted(&key));
                }
                out.labels.push(fitted(&key));
            }
        }
        Some(out)
    }

    /// The channels a matrix insert stands for, in order: each template
    /// channel for each pixel or group.
    fn matrix_channels(&self, insert: &Map<String, Value>) -> Option<Vec<String>> {
        let templates: Vec<&str> = insert.get("templateChannels")?.as_array()?.iter().filter_map(Value::as_str).collect();
        let keys = self.matrix.repeat_for(insert.get("repeatFor")?)?;
        if keys.len().saturating_mul(templates.len()) > UNIVERSE_SIZE {
            return None;
        }
        let per_pixel = insert.get("channelOrder").and_then(Value::as_str) != Some("perChannel");
        let name = |template: &str, key: &str| template.replace("$pixelKey", key);
        Some(if per_pixel {
            keys.iter().flat_map(|key| templates.iter().map(move |t| name(t, key))).collect()
        } else {
            templates.iter().flat_map(|t| keys.iter().map(move |key| name(t, key))).collect()
        })
    }

    /// The definition of channel `key`: a channel of its own, or a template
    /// channel with a pixel's key in it.
    fn definition(&self, key: &str) -> Option<&Value> {
        self.channels.get(key).or_else(|| self.templates.iter().find_map(|(template, channel)| fills(template, key).then_some(channel)))
    }

    fn is_fine(&self, key: &str) -> bool {
        self.fine.iter().any(|fine| *fine == key || fills(fine, key))
    }

    /// Our channel for `key`, and whether to check it on the fixture: it
    /// has no value to rest at that neither strobes, runs a program nor
    /// lights the fixture.
    fn channel(&self, key: &str) -> (Channel, bool) {
        if self.is_fine(key) {
            // The finer byte of a dimmer or position: its coarse byte is enough.
            return (Channel::Unused, false);
        }
        let Some(channel) = self.definition(key) else {
            // A switching channel, whose meaning another channel decides.
            return (Channel::Unused, false);
        };
        let shift = resolution_shift(channel);
        let capabilities = capabilities(channel, shift);
        if let Some(kind) = whole_range(&capabilities).or_else(|| shutter(&capabilities)).or_else(|| dimmer_below_the_top(&capabilities)) {
            return (kind, false);
        }
        let default = default_value(channel, shift);
        let at_default = capabilities.iter().find(|c| c.from <= default && default <= c.to);
        let rest = match at_default {
            Some(capability) if !capability.rests_safely(default) => capabilities.iter().find_map(Capability::safe_rest),
            _ => Some(default),
        };
        match rest {
            Some(value) => (held(value), false),
            None => (held(default), true),
        }
    }
}

/// Whether `key` is `template` with a pixel's key for its `$pixelKey`.
fn fills(template: &str, key: &str) -> bool {
    template.split_once("$pixelKey").is_some_and(|(before, after)| key.len() > before.len() + after.len() && key.starts_with(before) && key.ends_with(after))
}

/// A channel that always sends `value`.
fn held(value: u8) -> Channel {
    if value == 0 {
        Channel::Unused
    } else {
        Channel::Fixed { value }
    }
}

/// Most pixels a matrix may have: each takes a channel at least, and a
/// mode no more than a universe's.
const MAX_PIXELS: usize = UNIVERSE_SIZE;

/// A fixture's pixels and pixel groups, by the keys its matrix channels use.
#[derive(Default)]
struct Matrix {
    /// Each pixel's key and its x, y and z position from 1, sorted by key
    /// as the Open Fixture Library sorts them (numbers by value).
    pixels: Vec<(String, [usize; 3])>,
    /// The group keys, in the file's order.
    groups: Vec<String>,
}

impl Matrix {
    fn new(matrix: Option<&Value>, groups: Vec<String>) -> Matrix {
        let Some(matrix) = matrix else { return Matrix::default() };
        let structure: Vec<Vec<Vec<Option<String>>>> = match (matrix.get("pixelKeys"), matrix.get("pixelCount").and_then(Value::as_array)) {
            (Some(keys), _) => serde_json::from_value(keys.clone()).unwrap_or_default(),
            (None, Some(count)) => {
                let count: Vec<usize> = count.iter().map(|n| n.as_u64().map_or(1, |n| usize::try_from(n).unwrap_or(usize::MAX))).collect();
                let size = [0, 1, 2].map(|axis| count.get(axis).copied().unwrap_or(1));
                if size.iter().try_fold(1usize, |all, &n| all.checked_mul(n)).is_none_or(|all| all > MAX_PIXELS) {
                    return Matrix { pixels: Vec::new(), groups };
                }
                let [x, y, z] = size;
                (1..=z).map(|z_at| (1..=y).map(|y_at| (1..=x).map(|x_at| Some(default_key(size, [x_at, y_at, z_at]))).collect()).collect()).collect()
            }
            _ => Vec::new(),
        };
        let mut pixels = Vec::new();
        for (z, plane) in structure.into_iter().enumerate() {
            for (y, row) in plane.into_iter().enumerate() {
                for (x, key) in row.into_iter().enumerate() {
                    pixels.extend(key.map(|key| (key, [x + 1, y + 1, z + 1])));
                }
            }
        }
        pixels.sort_by(|(a, _), (b, _)| natural(a, b));
        Matrix { pixels, groups }
    }

    /// The pixel or group keys a matrix insert's `repeatFor` stands for.
    fn repeat_for(&self, repeat_for: &Value) -> Option<Vec<String>> {
        match repeat_for {
            Value::Array(keys) => Some(keys.iter().filter_map(Value::as_str).map(str::to_owned).collect()),
            Value::String(order) if order == "eachPixelGroup" => Some(self.groups.clone()),
            Value::String(order) if order == "eachPixelABC" => Some(self.pixels.iter().map(|(key, _)| key.clone()).collect()),
            Value::String(order) => {
                // "eachPixelXYZ": first by the last axis named, then the middle, then the first.
                let axes: Vec<usize> = order.strip_prefix("eachPixel")?.chars().map(|axis| "XYZ".find(axis)).collect::<Option<_>>()?;
                let [first, second, third] = <[usize; 3]>::try_from(axes).ok()?;
                let mut pixels = self.pixels.clone();
                pixels.sort_by_key(|(_, at)| (at[third], at[second], at[first]));
                Some(pixels.into_iter().map(|(key, _)| key).collect())
            }
            _ => None,
        }
    }
}

/// The key the Open Fixture Library gives the pixel at `at` in a matrix of
/// `size` pixels: its number along the one axis that has more than one,
/// "(x, y)" along two, "(x, y, z)" along three.
fn default_key(size: [usize; 3], [x, y, z]: [usize; 3]) -> String {
    let defined: Vec<usize> = (0..3).filter(|&axis| size[axis] > 1).collect();
    match defined.len() {
        2 => {
            let first = if defined.contains(&0) { x } else { y };
            let last = if defined.contains(&1) { y } else { z };
            format!("({first}, {last})")
        }
        3 => format!("({x}, {y}, {z})"),
        _ => x.max(y).max(z).to_string(),
    }
}

/// Orders keys with numbers in them by the numbers' value ("2" before
/// "10"), ignoring case.
fn natural(a: &str, b: &str) -> Ordering {
    fn parts(text: &str) -> Vec<(bool, String)> {
        let mut parts: Vec<(bool, String)> = Vec::new();
        for c in text.chars() {
            let digit = c.is_ascii_digit();
            match parts.last_mut() {
                Some((was_digit, part)) if *was_digit == digit => part.push(c),
                _ => parts.push((digit, c.to_string())),
            }
        }
        parts
    }
    let (a_parts, b_parts) = (parts(a), parts(b));
    for ((a_digit, a_part), (b_digit, b_part)) in a_parts.iter().zip(&b_parts) {
        let order = if *a_digit && *b_digit {
            let (a_number, b_number) = (a_part.trim_start_matches('0'), b_part.trim_start_matches('0'));
            a_number.len().cmp(&b_number.len()).then_with(|| a_number.cmp(b_number))
        } else {
            a_part.to_lowercase().cmp(&b_part.to_lowercase())
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a_parts.len().cmp(&b_parts.len()).then_with(|| a.cmp(b))
}

/// One range of a channel, in 8-bit values.
struct Capability<'a> {
    from: u8,
    to: u8,
    kind: &'a str,
    json: &'a Value,
}

impl Capability<'_> {
    fn text(&self, name: &str) -> Option<&str> {
        self.json.get(name).and_then(Value::as_str)
    }

    fn color(&self) -> Option<&str> {
        self.text("color")
    }

    /// Whether it gets darker as the value rises.
    fn falling(&self) -> bool {
        matches!(self.text("brightnessStart"), Some("bright" | "100%")) || matches!(self.text("brightnessEnd"), Some("off" | "dark" | "0%"))
    }

    /// Whether it gives no light at `value`: its brightness, or that at the
    /// end of its range `value` is at ("off" to "bright" unless it says).
    fn dark_at(&self, value: u8) -> bool {
        let off = |text: Option<&str>| matches!(text, Some("off" | "dark" | "0%"));
        match self.text("brightness") {
            Some(brightness) => off(Some(brightness)),
            None if value == self.from => off(self.text("brightnessStart").or(Some("off"))),
            None if value == self.to => off(self.text("brightnessEnd")),
            None => false,
        }
    }

    /// Whether it lights the fixture at `value`.
    fn lights(&self, value: u8) -> bool {
        matches!(self.kind, "Intensity" | "ColorIntensity") && !self.dark_at(value)
    }

    /// Whether it strobes (or pulses, flashes) at `value`: a shutter effect
    /// other than open or closed, at a speed other than stopped.
    fn strobes(&self, value: u8) -> bool {
        let stopped = |speed: Option<&str>| matches!(speed, Some("stop" | "0Hz" | "0%" | "0bpm"));
        self.kind == "ShutterStrobe"
            && !matches!(self.text("shutterEffect"), Some("Open" | "Closed"))
            && !stopped(self.text("speed"))
            && !(value == self.from && stopped(self.text("speedStart")))
            && !(value == self.to && stopped(self.text("speedEnd")))
    }

    /// Whether it runs a program: an effect that is a preset (a colour fade
    /// or jump), follows the sound, or by its name runs on its own.
    fn runs_a_program(&self) -> bool {
        const PROGRAM_WORDS: [&str; 17] =
            ["auto", "program", "prog.", "chase", "sound", "music", "random", "strobe", "fade", "jump", "iterate", "candle", "paparazzi", "macro", "show", "pattern", "pulse"];
        let name = self.text("effectName").unwrap_or_default().to_lowercase();
        self.kind == "Effect"
            && (self.json.get("effectPreset").is_some() || self.json.get("soundControlled").and_then(Value::as_bool) == Some(true) || PROGRAM_WORDS.iter().any(|word| name.contains(word)))
    }

    /// Whether a channel may rest at `value` in it: it does not strobe,
    /// run a program or light the fixture. A value other than 0 is held
    /// also in a blackout, so it counts as dark only where the definition
    /// says so, not by the "off" a range starts at unless it says.
    fn rests_safely(&self, value: u8) -> bool {
        let said = |name: &str| self.json.get(name).is_some();
        let unsaid_start = value > 0 && value == self.from && !said("brightness") && !said("brightnessStart");
        let lights = self.lights(value) || (unsaid_start && matches!(self.kind, "Intensity" | "ColorIntensity"));
        !(self.strobes(value) || self.runs_a_program() || lights)
    }

    /// A value in it a channel may rest at.
    fn safe_rest(&self) -> Option<u8> {
        [self.from, self.to].into_iter().find(|&value| self.rests_safely(value))
    }
}

/// How far to shift a channel's values to make them 8-bit: by a byte for
/// each fine channel, unless the channel says its values are 8-bit.
fn resolution_shift(channel: &Value) -> u32 {
    let fine = channel.get("fineChannelAliases").and_then(Value::as_array).map_or(0, Vec::len) as u32;
    let bytes = match channel.get("dmxValueResolution").and_then(Value::as_str) {
        Some(resolution) => resolution.trim_end_matches("bit").parse::<u32>().map_or(1, |bits| bits / 8),
        None => 1 + fine,
    };
    8 * bytes.saturating_sub(1).min(7)
}

fn capabilities(channel: &Value, shift: u32) -> Vec<Capability<'_>> {
    let eight_bit = |value: &Value| u8::try_from(value.as_u64().unwrap_or(0).checked_shr(shift).unwrap_or(0)).unwrap_or(u8::MAX);
    fn read(json: &Value, eight_bit: impl Fn(&Value) -> u8) -> Option<Capability<'_>> {
        let (from, to) = match json.get("dmxRange").and_then(Value::as_array) {
            Some(range) if range.len() == 2 => (eight_bit(&range[0]), eight_bit(&range[1])),
            _ => (0, u8::MAX),
        };
        Some(Capability { from, to, kind: json.get("type")?.as_str()?, json })
    }
    match (channel.get("capability"), channel.get("capabilities").and_then(Value::as_array)) {
        (Some(one), _) => read(one, eight_bit).into_iter().collect(),
        (None, Some(many)) => many.iter().filter_map(|c| read(c, eight_bit)).collect(),
        _ => Vec::new(),
    }
}

/// Our channel for one that dims, or shows one colour, over its whole
/// range: from dark, or for a dimmer also to dark.
fn whole_range(capabilities: &[Capability]) -> Option<Channel> {
    let first = capabilities.first()?;
    let whole = first.from == 0 && capabilities.last()?.to == u8::MAX;
    let inverted = capabilities.iter().all(|c| c.kind == "Intensity" && c.falling());
    if whole && inverted && capabilities.last()?.dark_at(u8::MAX) {
        return Some(Channel::DimmerRange { dark: u8::MAX, bright: 0 });
    }
    let kind = match (first.kind, first.color()) {
        ("Intensity", _) => Channel::Dimmer,
        ("ColorIntensity", Some("Red")) => Channel::Red,
        ("ColorIntensity", Some("Green")) => Channel::Green,
        ("ColorIntensity", Some("Blue")) => Channel::Blue,
        ("ColorIntensity", Some("White" | "Warm White" | "Cold White")) => Channel::White,
        _ => return None,
    };
    let same = capabilities.iter().all(|c| c.kind == first.kind && c.color() == first.color() && !c.falling());
    (same && whole && first.dark_at(0)).then_some(kind)
}

/// A shutter: its first "Open" value, or a lamp switch's "bright" one, and
/// its first "Closed" one, or else the start of a range that dims from dark.
/// A lamp switch only switches: a channel that also dims is a dimmer.
fn shutter(capabilities: &[Capability]) -> Option<Channel> {
    let effect = |name: &str| capabilities.iter().find(|c| c.kind == "ShutterStrobe" && c.text("shutterEffect") == Some(name));
    let only_switches = capabilities.iter().filter(|c| c.kind == "Intensity").all(|c| c.text("brightness").is_some());
    let switched_on = || capabilities.iter().find(|c| only_switches && c.kind == "Intensity" && c.text("brightness").is_some_and(|b| b == "bright" || b == "100%"));
    let open = effect("Open").or_else(switched_on)?.from;
    let dims_from_dark = || capabilities.iter().find(|c| c.kind == "Intensity" && c.dark_at(c.from));
    let closed = effect("Closed").or_else(dims_from_dark)?.from;
    Some(Channel::Shutter { open, closed })
}

/// A channel that dims from dark at 0 and does something else above: our
/// dimmer, up to its brightest value before that.
fn dimmer_below_the_top(capabilities: &[Capability]) -> Option<Channel> {
    let dims: Vec<&Capability> = capabilities.iter().take_while(|c| c.kind == "Intensity" && !c.falling()).collect();
    let (first, last) = (dims.first()?, dims.last()?);
    let lit = dims.iter().any(|c| c.lights(c.to));
    (first.from == 0 && first.dark_at(0) && lit).then_some(Channel::DimmerRange { dark: 0, bright: last.to })
}

/// The value a channel rests at when nothing drives it: a number, or a
/// percentage of its range.
fn default_value(channel: &Value, shift: u32) -> u8 {
    match channel.get("defaultValue") {
        Some(Value::Number(n)) => u8::try_from(n.as_u64().unwrap_or(0).checked_shr(shift).unwrap_or(0)).unwrap_or(u8::MAX),
        Some(Value::String(percent)) => {
            let share = percent.trim_end_matches('%').trim().parse::<f64>().unwrap_or(0.0).clamp(0.0, 100.0);
            (share / 100.0 * 255.0).round() as u8
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(definition: &Value) -> LibraryFixture {
        match convert("Test", "test/fixture", &definition.to_string()).unwrap() {
            Converted::Fixture(fixture) => fixture,
            other => panic!("{other:?}"),
        }
    }

    fn channels(definition: Value) -> Vec<Channel> {
        fixture(&definition).modes.remove(0).channels
    }

    #[test]
    fn dimmer_and_colour_channels_become_ours_and_a_shutter_opens_with_the_look() {
        let definition = json!({
            "name": "Par",
            "availableChannels": {
                "Dimmer": { "capability": { "type": "Intensity" } },
                "Red": { "capability": { "type": "ColorIntensity", "color": "Red" } },
                "Green": { "capability": { "type": "ColorIntensity", "color": "Green" } },
                "Blue": { "capability": { "type": "ColorIntensity", "color": "Blue" } },
                "Warm White": { "capability": { "type": "ColorIntensity", "color": "Warm White" } },
                "Amber": { "capability": { "type": "ColorIntensity", "color": "Amber" } },
                "Shutter": { "capabilities": [
                    { "dmxRange": [0, 9], "type": "ShutterStrobe", "shutterEffect": "Closed" },
                    { "dmxRange": [10, 19], "type": "ShutterStrobe", "shutterEffect": "Open" },
                    { "dmxRange": [20, 255], "type": "ShutterStrobe", "shutterEffect": "Strobe" }
                ] }
            },
            "modes": [{ "name": "7-channel", "channels": ["Dimmer", "Red", "Green", "Blue", "Warm White", "Amber", "Shutter"] }]
        });
        let modes = fixture(&definition).modes;
        assert_eq!(
            modes[0].channels,
            [Channel::Dimmer, Channel::Red, Channel::Green, Channel::Blue, Channel::White, Channel::Unused, Channel::Shutter { open: 10, closed: 0 }]
        );
        assert_eq!(modes[0].labels[6], "Shutter");
        assert!(modes[0].check.is_empty());
    }

    #[test]
    fn a_shutter_closes_on_a_range_that_dims_from_dark_and_needs_a_dark_value() {
        let definition = json!({
            "name": "Spot",
            "availableChannels": {
                "Shutter / Dimmer": { "capabilities": [
                    { "dmxRange": [0, 9], "type": "ShutterStrobe", "shutterEffect": "Open" },
                    { "dmxRange": [10, 99], "type": "Intensity" },
                    { "dmxRange": [100, 255], "type": "ShutterStrobe", "shutterEffect": "Strobe", "speedStart": "slow", "speedEnd": "fast" }
                ] },
                "Strobe": { "capabilities": [
                    { "dmxRange": [0, 5], "type": "ShutterStrobe", "shutterEffect": "Open" },
                    { "dmxRange": [6, 255], "type": "ShutterStrobe", "shutterEffect": "Strobe", "speedStart": "slow", "speedEnd": "fast" }
                ] }
            },
            "modes": [{ "name": "2-channel", "channels": ["Shutter / Dimmer", "Strobe"] }]
        });
        // Without a dark value the strobe channel is no shutter: it rests open, at 0.
        assert_eq!(channels(definition), [Channel::Shutter { open: 0, closed: 10 }, Channel::Unused]);
    }

    #[test]
    fn a_dimmer_that_strobes_at_its_top_dims_below_it_and_others_rest_at_their_default() {
        let definition = json!({
            "name": "Spot",
            "availableChannels": {
                "Dimmer": { "capabilities": [
                    { "dmxRange": [0, 189], "type": "Intensity" },
                    { "dmxRange": [190, 255], "type": "ShutterStrobe", "shutterEffect": "Strobe" }
                ] },
                "Pan": { "defaultValue": "50%", "fineChannelAliases": ["Pan fine"], "capability": { "type": "Pan", "angleStart": "0deg", "angleEnd": "540deg" } },
                "Tilt": { "defaultValue": 32768, "fineChannelAliases": ["Tilt fine"], "capability": { "type": "Tilt" } },
                "Reset": { "capabilities": [
                    { "dmxRange": [0, 249], "type": "NoFunction" },
                    { "dmxRange": [250, 255], "type": "Maintenance" }
                ] }
            },
            "modes": [{ "name": "Extended", "channels": ["Pan", "Pan fine", "Tilt", "Tilt fine", "Dimmer", null, "Reset", "Speed / Sound"] }]
        });
        assert_eq!(
            channels(definition),
            [
                Channel::Fixed { value: 128 },
                Channel::Unused,
                Channel::Fixed { value: 128 },
                Channel::Unused,
                Channel::DimmerRange { dark: 0, bright: 189 },
                Channel::Unused,
                Channel::Unused,
                Channel::Unused
            ]
        );
    }

    #[test]
    fn a_channel_rests_where_it_neither_strobes_nor_runs_an_effect_nor_lights_or_is_listed_to_check() {
        let definition = json!({
            "name": "Bar",
            "availableChannels": {
                "Dimmer": { "capability": { "type": "Intensity" } },
                "Strobe": { "capabilities": [
                    { "dmxRange": [0, 249], "type": "ShutterStrobe", "shutterEffect": "Strobe", "speedStart": "fast", "speedEnd": "slow" },
                    { "dmxRange": [250, 255], "type": "NoFunction" }
                ] },
                "Strobe Speed": { "capability": { "type": "ShutterStrobe", "shutterEffect": "Strobe", "speedStart": "1Hz", "speedEnd": "20Hz" } },
                "Stopped Strobe": { "capability": { "type": "ShutterStrobe", "shutterEffect": "Strobe", "speedStart": "stop", "speedEnd": "fast" } },
                "Mode": { "defaultValue": 10, "capabilities": [
                    { "dmxRange": [0, 9], "type": "Intensity", "brightness": "off" },
                    { "dmxRange": [10, 255], "type": "Effect", "effectName": "Auto show" }
                ] },
                "Program": { "capability": { "type": "Effect", "effectName": "Sound program", "soundControlled": true } },
                "Programs": { "defaultValue": 20, "capabilities": [
                    { "dmxRange": [0, 9], "type": "NoFunction" },
                    { "dmxRange": [10, 50], "type": "Intensity" },
                    { "dmxRange": [51, 255], "type": "Effect", "effectName": "Program" }
                ] }
            },
            "modes": [{ "name": "7-channel", "channels": ["Dimmer", "Strobe", "Strobe Speed", "Stopped Strobe", "Mode", "Program", "Programs"] }]
        });
        let mode = fixture(&definition).modes.remove(0);
        assert_eq!(
            mode.channels,
            [Channel::Dimmer, Channel::Fixed { value: 250 }, Channel::Unused, Channel::Unused, Channel::Unused, Channel::Unused, Channel::Unused]
        );
        assert_eq!(mode.check, ["Strobe Speed", "Program"]);
    }

    #[test]
    fn matrix_channels_repeat_for_each_pixel_in_order() {
        let definition = json!({
            "name": "Bar",
            "matrix": { "pixelCount": [2, 1, 1], "pixelGroups": { "Master": "all" } },
            "availableChannels": {},
            "templateChannels": {
                "Red $pixelKey": { "capability": { "type": "ColorIntensity", "color": "Red" } },
                "Green $pixelKey": { "capability": { "type": "ColorIntensity", "color": "Green" } }
            },
            "modes": [
                { "name": "Master", "channels": ["Red Master", "Green Master"] },
                { "name": "Per pixel", "channels": [{ "insert": "matrixChannels", "repeatFor": "eachPixelABC", "channelOrder": "perPixel", "templateChannels": ["Red $pixelKey", "Green $pixelKey"] }] },
                { "name": "Per colour", "channels": [{ "insert": "matrixChannels", "repeatFor": ["1", "2"], "channelOrder": "perChannel", "templateChannels": ["Red $pixelKey", "Green $pixelKey"] }] }
            ]
        });
        let modes = fixture(&definition).modes;
        assert_eq!(modes[0].channels, [Channel::Red, Channel::Green]);
        assert_eq!(modes[1].labels, ["Red 1", "Green 1", "Red 2", "Green 2"]);
        assert_eq!(modes[1].channels, [Channel::Red, Channel::Green, Channel::Red, Channel::Green]);
        assert_eq!(modes[2].labels, ["Red 1", "Red 2", "Green 1", "Green 2"]);
    }

    #[test]
    fn matrix_labels_use_the_pixel_and_group_keys_and_fine_template_bytes_rest() {
        // The groups are not in alphabetical order in the file.
        let text = r#"{
            "name": "Ring",
            "matrix": {
                "pixelKeys": [[["Top", null], ["Left", "Right"]]],
                "pixelGroups": { "Outer": ["Left", "Right"], "Inner": ["Top"] }
            },
            "availableChannels": {},
            "templateChannels": {
                "Dimmer $pixelKey": { "fineChannelAliases": ["Dimmer $pixelKey fine"], "capability": { "type": "Intensity" } }
            },
            "modes": [
                { "name": "Groups", "channels": [{ "insert": "matrixChannels", "repeatFor": "eachPixelGroup", "channelOrder": "perPixel", "templateChannels": ["Dimmer $pixelKey"] }] },
                { "name": "Pixels", "channels": [{ "insert": "matrixChannels", "repeatFor": "eachPixelYXZ", "channelOrder": "perPixel", "templateChannels": ["Dimmer $pixelKey", "Dimmer $pixelKey fine"] }] }
            ]
        }"#;
        let Converted::Fixture(fixture) = convert("Test", "test/ring", text).unwrap() else { panic!("not converted") };
        assert_eq!(fixture.modes[0].labels, ["Dimmer Outer", "Dimmer Inner"]);
        assert_eq!(fixture.modes[1].labels, ["Dimmer Top", "Dimmer Top fine", "Dimmer Left", "Dimmer Left fine", "Dimmer Right", "Dimmer Right fine"]);
        assert_eq!(fixture.modes[1].channels, [Channel::Dimmer, Channel::Unused, Channel::Dimmer, Channel::Unused, Channel::Dimmer, Channel::Unused]);
    }

    #[test]
    fn pixel_keys_and_their_order_follow_the_open_fixture_library() {
        let keys = |size: Value, order: &str| Matrix::new(Some(&json!({ "pixelCount": size })), Vec::new()).repeat_for(&json!(order)).unwrap();
        let twelve = keys(json!([12, 1, 1]), "eachPixelABC");
        assert_eq!([&twelve[0], &twelve[1], &twelve[9], &twelve[11]], ["1", "2", "10", "12"]);
        assert_eq!(keys(json!([2, 2, 1]), "eachPixelXYZ"), ["(1, 1)", "(2, 1)", "(1, 2)", "(2, 2)"]);
        assert_eq!(keys(json!([2, 2, 1]), "eachPixelYXZ"), ["(1, 1)", "(1, 2)", "(2, 1)", "(2, 2)"]);
        assert_eq!(keys(json!([1, 1, 2]), "eachPixelABC"), ["1", "2"]);
        let holes = Matrix::new(Some(&json!({ "pixelKeys": [[["b", null, "a10"]], [["a9"]]] })), Vec::new());
        assert_eq!(holes.repeat_for(&json!("eachPixelABC")).unwrap(), ["a9", "a10", "b"]);
    }

    #[test]
    fn an_inverted_dimmer_dims_the_other_way_and_a_lamp_switch_is_a_shutter() {
        let definition = json!({
            "name": "Scanner",
            "availableChannels": {
                "Dimmer": { "capability": { "type": "Intensity", "brightnessStart": "bright", "brightnessEnd": "off" } },
                "Lamp": { "capabilities": [
                    { "dmxRange": [0, 0], "type": "Intensity", "brightness": "off" },
                    { "dmxRange": [1, 254], "type": "NoFunction" },
                    { "dmxRange": [255, 255], "type": "Intensity", "brightness": "bright" }
                ] }
            },
            "modes": [{ "name": "2-channel", "channels": ["Dimmer", "Lamp"] }]
        });
        assert_eq!(channels(definition), [Channel::DimmerRange { dark: 255, bright: 0 }, Channel::Shutter { open: 255, closed: 0 }]);
    }

    #[test]
    fn any_definition_gives_a_result_or_none_and_never_panics() {
        let odd = json!({
            "name": "Odd",
            "matrix": { "pixelCount": [100000, 100000, 100000] },
            "availableChannels": {
                "Wide": { "dmxValueResolution": "4096bit", "defaultValue": 18446744073709551615u64, "capability": { "type": "Intensity" } },
                "Many": { "fineChannelAliases": ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"], "capability": { "type": "Pan" } }
            },
            "templateChannels": { "Red $pixelKey": { "capability": { "type": "ColorIntensity", "color": "Red" } } },
            "modes": [
                { "name": "Plain", "channels": ["Wide", "Many"] },
                { "name": "Huge", "channels": [{ "insert": "matrixChannels", "repeatFor": "eachPixelXYZ", "channelOrder": "perPixel", "templateChannels": ["Red $pixelKey"] }] },
                { "name": "Too many", "channels": [{ "insert": "matrixChannels", "repeatFor": (0..600).map(|n| n.to_string()).collect::<Vec<_>>(), "channelOrder": "perPixel", "templateChannels": ["Red $pixelKey"] }] }
            ]
        });
        let modes = fixture(&odd).modes;
        assert_eq!(modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Plain"]);
        for text in ["[1, 2", "[1, 2]", r#"{ "name": "x", "matrix": 5, "modes": [] }"#, &"[".repeat(10_000)] {
            assert!(convert("Test", "test/x", text).is_err(), "{text:.20}");
        }
    }

    #[test]
    fn a_redirect_a_laser_or_a_fixture_without_modes_is_left_out() {
        let convert = |definition: Value| convert("Test", "test/x", &definition.to_string()).unwrap();
        assert_eq!(convert(json!({ "name": "Old", "redirectTo": "test/new", "reason": "SameAsDifferentBrand" })), Converted::Redirect);
        assert_eq!(convert(json!({ "name": "None", "availableChannels": {}, "modes": [] })), Converted::NoModes);
        let laser = json!({
            "name": "Effect", "categories": ["Effect", "Laser"],
            "availableChannels": { "Dimmer": { "capability": { "type": "Intensity" } } },
            "modes": [{ "name": "1-channel", "channels": ["Dimmer"] }]
        });
        assert_eq!(convert(laser), Converted::Laser);
    }
}
