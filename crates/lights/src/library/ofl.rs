//! Turns an Open Fixture Library fixture definition (its JSON format,
//! <https://github.com/OpenLightingProject/open-fixture-library/blob/master/docs/fixture-format.md>)
//! into our library's modes. Used only by `examples/ofl-import.rs`, when
//! the library is made again; the app never reads this format.
//!
//! Every channel becomes one of ours, never something that flashes or
//! moves on its own:
//!
//! - one whose whole range dims (`Intensity`) is a dimmer, and one whose
//!   whole range is red, green, blue or white (`ColorIntensity`) that colour;
//! - a shutter opens while a look lights the fixture and closes otherwise
//!   (`Shutter`, never its strobe range), and a dimmer whose top range
//!   strobes is held at its brightest value below it: the app strobes only
//!   through a look's strobe effect, which is limited (`MIN_EFFECT_MS`);
//! - anything else (pan, tilt, gobos, programs, fog, maintenance) holds its
//!   default value, which is what the fixture does when no desk drives it.
//!
//! The manual's name for each channel goes along as its label, so the
//! quizmaster can still set it by hand in a look's extra channels.

use super::{LibraryFixture, LibraryMode};
use crate::setup::{Channel, MAX_FIXTURE_CHANNELS, MAX_LIGHTS_NAME_CHARS};
use crate::UNIVERSE_SIZE;
use serde_json::{Map, Value};

/// `fixture` (one definition file's JSON) of `manufacturer`, at `key`
/// ("manufacturer/model"); none for a redirect to another definition.
/// Modes with more channels than a fixture may have are left out.
pub fn convert(manufacturer: &str, key: &str, fixture: &Value) -> Option<LibraryFixture> {
    if fixture.get("redirectTo").is_some() {
        return None;
    }
    let definition = Definition::new(fixture);
    let modes = fixture
        .get("modes")?
        .as_array()?
        .iter()
        .filter_map(|mode| {
            let (channels, labels) = definition.mode(mode)?;
            let name = fitted(mode.get("name")?.as_str()?);
            (!channels.is_empty() && channels.len() <= MAX_FIXTURE_CHANNELS.min(UNIVERSE_SIZE)).then_some(LibraryMode { name, channels, labels })
        })
        .collect::<Vec<_>>();
    let name = fitted(fixture.get("name")?.as_str()?);
    (!modes.is_empty()).then(|| LibraryFixture { manufacturer: manufacturer.to_owned(), name, key: key.to_owned(), modes })
}

/// `text` cut to a name's length.
fn fitted(text: &str) -> String {
    text.chars().take(MAX_LIGHTS_NAME_CHARS).collect()
}

struct Definition<'a> {
    channels: Map<String, Value>,
    templates: Map<String, Value>,
    /// The fixture's pixels and pixel groups, for matrix channels.
    pixels: usize,
    groups: usize,
    fine: Vec<&'a str>,
}

impl<'a> Definition<'a> {
    fn new(fixture: &'a Value) -> Definition<'a> {
        let object = |name: &str| fixture.get(name).and_then(Value::as_object).cloned().unwrap_or_default();
        let channels = object("availableChannels");
        let fine = fixture
            .get("availableChannels")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|channels| channels.values())
            .filter_map(|channel| channel.get("fineChannelAliases")?.as_array())
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let matrix = fixture.get("matrix");
        let pixels = matrix.map_or(0, pixel_count);
        let groups = matrix.and_then(|m| m.get("pixelGroups")).and_then(Value::as_object).map_or(0, Map::len);
        Definition { channels, templates: object("templateChannels"), pixels, groups, fine }
    }

    /// A mode's channels and their labels; none if a matrix in it cannot be read.
    fn mode(&self, mode: &Value) -> Option<(Vec<Channel>, Vec<String>)> {
        let mut channels = Vec::new();
        let mut labels = Vec::new();
        for entry in mode.get("channels")?.as_array()? {
            match entry {
                Value::String(key) => {
                    channels.push(self.channel(key));
                    labels.push(fitted(key));
                }
                Value::Object(insert) => {
                    for key in self.matrix_channels(insert)? {
                        channels.push(self.channel(&key));
                        labels.push(fitted(&key));
                    }
                }
                // A channel the mode leaves out.
                _ => {
                    channels.push(Channel::Unused);
                    labels.push(String::new());
                }
            }
            if channels.len() > UNIVERSE_SIZE {
                return None;
            }
        }
        Some((channels, labels))
    }

    /// The channels a matrix insert stands for, in order: each template
    /// channel for each pixel or group.
    fn matrix_channels(&self, insert: &Map<String, Value>) -> Option<Vec<String>> {
        let templates: Vec<&str> = insert.get("templateChannels")?.as_array()?.iter().filter_map(Value::as_str).collect();
        let keys: Vec<String> = match insert.get("repeatFor")? {
            Value::Array(keys) => keys.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
            Value::String(order) if order == "eachPixelGroup" => (1..=self.groups).map(|n| format!("Group {n}")).collect(),
            Value::String(_) => (1..=self.pixels).map(|n| n.to_string()).collect(),
            _ => return None,
        };
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
        self.channels.get(key).or_else(|| {
            self.templates.iter().find_map(|(template, channel)| {
                let (before, after) = template.split_once("$pixelKey")?;
                (key.len() > before.len() + after.len() && key.starts_with(before) && key.ends_with(after)).then_some(channel)
            })
        })
    }

    fn channel(&self, key: &str) -> Channel {
        if self.fine.contains(&key) {
            // The finer byte of a dimmer or position: its coarse byte is enough.
            return Channel::Unused;
        }
        let Some(channel) = self.definition(key) else {
            // A switching channel, whose meaning another channel decides.
            return Channel::Unused;
        };
        let shift = resolution_shift(channel);
        let capabilities = capabilities(channel, shift);
        let held = |value: u8| if value == 0 { Channel::Unused } else { Channel::Fixed { value } };
        if let Some(kind) = whole_range(&capabilities) {
            return kind;
        }
        let shutter = |effect: &str| capabilities.iter().find(|c| c.kind == "ShutterStrobe" && c.shutter == Some(effect));
        if let Some(open) = shutter("Open") {
            return Channel::Shutter { open: open.from, closed: shutter("Closed").map_or(0, |closed| closed.from) };
        }
        if let Some(bright) = brightest_dimming(&capabilities) {
            return held(bright);
        }
        held(default_value(channel, shift))
    }
}

/// How many pixels a matrix has: its pixel keys that are not empty, or the
/// product of its pixel counts.
fn pixel_count(matrix: &Value) -> usize {
    if let Some(count) = matrix.get("pixelCount").and_then(Value::as_array) {
        return count.iter().filter_map(Value::as_u64).product::<u64>() as usize;
    }
    let keys = matrix.get("pixelKeys").and_then(Value::as_array);
    keys.into_iter().flatten().filter_map(Value::as_array).flatten().filter_map(Value::as_array).flatten().filter(|key| key.is_string()).count()
}

/// One range of a channel, in 8-bit values.
struct Capability<'a> {
    from: u8,
    to: u8,
    kind: &'a str,
    color: Option<&'a str>,
    shutter: Option<&'a str>,
    /// Whether it gets darker as the value rises.
    falling: bool,
}

/// How far to shift a channel's values to make them 8-bit: by a byte for
/// each fine channel, unless the channel says its values are 8-bit.
fn resolution_shift(channel: &Value) -> u32 {
    let fine = channel.get("fineChannelAliases").and_then(Value::as_array).map_or(0, Vec::len) as u32;
    let bytes = match channel.get("dmxValueResolution").and_then(Value::as_str) {
        Some(resolution) => resolution.trim_end_matches("bit").parse::<u32>().map_or(1, |bits| bits / 8),
        None => 1 + fine,
    };
    8 * bytes.saturating_sub(1)
}

fn capabilities(channel: &Value, shift: u32) -> Vec<Capability<'_>> {
    let eight_bit = |value: &Value| u8::try_from(value.as_u64().unwrap_or(0) >> shift).unwrap_or(u8::MAX);
    fn read(capability: &Value, eight_bit: impl Fn(&Value) -> u8) -> Option<Capability<'_>> {
        let (from, to) = match capability.get("dmxRange").and_then(Value::as_array) {
            Some(range) if range.len() == 2 => (eight_bit(&range[0]), eight_bit(&range[1])),
            _ => (0, u8::MAX),
        };
        let text = |name: &str| capability.get(name).and_then(Value::as_str);
        let falling = matches!(text("brightnessStart"), Some("bright" | "100%")) || matches!(text("brightnessEnd"), Some("off" | "dark" | "0%"));
        Some(Capability { from, to, kind: text("type")?, color: text("color"), shutter: text("shutterEffect"), falling })
    }
    match (channel.get("capability"), channel.get("capabilities").and_then(Value::as_array)) {
        (Some(one), _) => read(one, eight_bit).into_iter().collect(),
        (None, Some(many)) => many.iter().filter_map(|c| read(c, eight_bit)).collect(),
        _ => Vec::new(),
    }
}

/// Our channel for one that dims, or shows one colour, over its whole range.
fn whole_range(capabilities: &[Capability]) -> Option<Channel> {
    let first = capabilities.first()?;
    let kind = match (first.kind, first.color) {
        ("Intensity", _) => Channel::Dimmer,
        ("ColorIntensity", Some("Red")) => Channel::Red,
        ("ColorIntensity", Some("Green")) => Channel::Green,
        ("ColorIntensity", Some("Blue")) => Channel::Blue,
        ("ColorIntensity", Some("White" | "Warm White" | "Cold White")) => Channel::White,
        _ => return None,
    };
    let same = capabilities.iter().all(|c| c.kind == first.kind && c.color == first.color && !c.falling);
    let whole = first.from == 0 && capabilities.last()?.to == u8::MAX;
    (same && whole).then_some(kind)
}

/// The brightest value of a channel that dims from 0 and then does
/// something else, such as strobe, above it.
fn brightest_dimming(capabilities: &[Capability]) -> Option<u8> {
    let dims = capabilities.iter().take_while(|c| c.kind == "Intensity" && !c.falling);
    let last = dims.last()?;
    (capabilities[0].from == 0).then_some(last.to)
}

/// The value a channel rests at when nothing drives it: a number, or a
/// percentage of its range.
fn default_value(channel: &Value, shift: u32) -> u8 {
    match channel.get("defaultValue") {
        Some(Value::Number(n)) => u8::try_from(n.as_u64().unwrap_or(0) >> shift).unwrap_or(u8::MAX),
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

    fn channels(fixture: Value) -> Vec<Channel> {
        convert("Test", "test/fixture", &fixture).unwrap().modes.remove(0).channels
    }

    #[test]
    fn dimmer_and_colour_channels_become_ours_and_a_shutter_opens_with_the_look() {
        let fixture = json!({
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
        let modes = convert("Test", "test/par", &fixture).unwrap().modes;
        assert_eq!(
            modes[0].channels,
            [Channel::Dimmer, Channel::Red, Channel::Green, Channel::Blue, Channel::White, Channel::Unused, Channel::Shutter { open: 10, closed: 0 }]
        );
        assert_eq!(modes[0].labels[6], "Shutter");
    }

    #[test]
    fn a_dimmer_that_strobes_at_its_top_is_held_bright_and_others_rest_at_their_default() {
        let fixture = json!({
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
            channels(fixture),
            [
                Channel::Fixed { value: 128 },
                Channel::Unused,
                Channel::Fixed { value: 128 },
                Channel::Unused,
                Channel::Fixed { value: 189 },
                Channel::Unused,
                Channel::Unused,
                Channel::Unused
            ]
        );
    }

    #[test]
    fn matrix_channels_repeat_for_each_pixel_in_order() {
        let fixture = json!({
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
        let modes = convert("Test", "test/bar", &fixture).unwrap().modes;
        assert_eq!(modes[0].channels, [Channel::Red, Channel::Green]);
        assert_eq!(modes[1].labels, ["Red 1", "Green 1", "Red 2", "Green 2"]);
        assert_eq!(modes[1].channels, [Channel::Red, Channel::Green, Channel::Red, Channel::Green]);
        assert_eq!(modes[2].labels, ["Red 1", "Red 2", "Green 1", "Green 2"]);
    }

    #[test]
    fn a_redirect_or_a_fixture_without_modes_is_left_out() {
        assert!(convert("Test", "test/old", &json!({ "name": "Old", "redirectTo": "test/new", "reason": "SameAsDifferentBrand" })).is_none());
        assert!(convert("Test", "test/none", &json!({ "name": "None", "availableChannels": {}, "modes": [] })).is_none());
    }
}
