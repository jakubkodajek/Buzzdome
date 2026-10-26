//! The venue's lights as the quizmaster sets them up, kept in the app's
//! settings rather than in a quiz: the same quiz plays in rooms with other
//! lights. Every value is checked when it comes in (`LightsSetup::check`),
//! so the lights thread never meets an address outside the universe.

use crate::dmx::UNIVERSE_SIZE;
use quiz_core::{Color, Cue, Stage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;
use ts_rs::TS;

pub type FixtureId = u32;
pub type LookId = u32;

/// Most fixtures a setup has.
pub const MAX_FIXTURES: usize = 128;
/// Most channels one fixture takes: a whole universe, as a pixel bar's
/// widest mode may.
pub const MAX_FIXTURE_CHANNELS: usize = crate::dmx::UNIVERSE_SIZE;
/// Most looks a setup has.
pub const MAX_LOOKS: usize = 64;
/// Most colours a look runs through.
pub const MAX_LOOK_COLORS: usize = 8;
/// Most channels a look sets by address.
pub const MAX_LOOK_CHANNELS: usize = 64;
/// Longest name of a fixture or look, in characters.
pub const MAX_LIGHTS_NAME_CHARS: usize = 60;
/// Fastest and slowest step of a look's effect, in milliseconds. A strobe
/// lights once a step, a pulse dims and lights once, a chase changes colour
/// once: at 500 ms that is at most 2 flashes a second, under the 3 a second
/// that can trigger photosensitive seizures (W3C, Understanding WCAG 2.0 SC
/// 2.3.1, <https://www.w3.org/WAI/WCAG20/Understanding/three-flashes-or-below-threshold>)
/// and the 4 a second the UK Health and Safety Executive allows strobes at
/// public events (Epilepsy Society, Photosensitive epilepsy, April 2025,
/// <https://epilepsysociety.org.uk/sites/default/files/2025-04/PhotosensitiveepilepsyApril2025.pdf>).
pub const MIN_EFFECT_MS: u32 = 500;
pub const MAX_EFFECT_MS: u32 = 10_000;
/// Longest fade into a look, in milliseconds.
pub const MAX_FADE_MS: u32 = 10_000;
/// Shortest and longest flash of a cue, in milliseconds.
pub const MIN_FLASH_MS: u32 = 100;
pub const MAX_FLASH_MS: u32 = 10_000;
/// The highest Art-Net port-address (15 bits: net, sub-net, universe).
pub const MAX_ARTNET_UNIVERSE: u16 = 32_767;
/// The sACN universes a source may send (ANSI E1.31).
pub const MIN_SACN_UNIVERSE: u16 = 1;
pub const MAX_SACN_UNIVERSE: u16 = 63_999;
/// The most of a host that is not an IPv4 address an error repeats.
pub const MAX_HOST_CHARS: usize = 253;
/// Longest name of a serial port or MIDI port.
pub const MAX_PORT_CHARS: usize = 200;

/// One DMX channel of a fixture, in the order the fixture takes them from
/// its start address. A look fills them from its colour and intensity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Channel {
    /// The fixture's own dimmer: the look's intensity. A fixture with one
    /// gets its colour at full.
    Dimmer,
    /// A dimmer that does something else above `max`, such as strobe or run
    /// a program: the look's intensity, from 0 up to `max`.
    DimmerUpTo { max: u8 },
    Red,
    Green,
    Blue,
    /// The white part of the look's colour (the least of red, green and blue).
    White,
    /// The look's strobe value, as the fixture reads it; 0 when it has none.
    Strobe,
    /// A shutter: `open` while the look lights the fixture, `closed` while
    /// it is dark (blackout, a look without it, a strobe's dark half).
    Shutter { open: u8, closed: u8 },
    /// Always this value, also in a blackout: a fixture's mode or a moving
    /// head's position.
    Fixed { value: u8 },
    /// Always 0.
    Unused,
}

/// A light at a DMX address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    /// Its first channel, from 1.
    pub address: u16,
    pub channels: Vec<Channel>,
    /// What the manual calls each channel, from the fixture library; may be
    /// shorter than `channels`, or empty for a fixture typed in by hand.
    #[serde(default)]
    pub labels: Vec<String>,
}

impl Fixture {
    /// The address of its last channel.
    fn last_address(&self) -> usize {
        usize::from(self.address) + self.channels.len().saturating_sub(1)
    }
}

/// How a look moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LookEffect {
    /// Each fixture keeps one of the look's colours, in turn.
    Static,
    /// The colours move on from fixture to fixture every step.
    Chase,
    /// The intensity swells and fades once a step.
    Pulse,
    /// The fixtures flash on and off once a step.
    Strobe,
}

/// A channel a look sets by its address, such as a smoke machine's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ChannelValue {
    pub address: u16,
    pub value: u8,
}

/// What the lights show at a moment of the show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Look {
    pub id: LookId,
    pub name: String,
    /// One colour or more, given to the fixtures in turn.
    pub colors: Vec<Color>,
    /// In percent.
    pub intensity: u8,
    pub effect: LookEffect,
    /// One step of the effect, in milliseconds.
    pub step_ms: u32,
    /// How long the lights take to go over to this look.
    pub fade_ms: u32,
    /// The fixtures it lights; none means all. The others go dark.
    pub fixtures: Vec<FixtureId>,
    /// The value of the fixtures' strobe channels.
    pub strobe: u8,
    /// Channels it sets by address, after the fixtures.
    pub channels: Vec<ChannelValue>,
}

/// A look shown for a while when a cue happens, then the stage's look again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Flash {
    pub look: LookId,
    pub ms: u32,
}

/// Where the lights go out.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputChoice {
    /// The lights are only drawn in the Director.
    #[default]
    None,
    /// Enttec DMX USB Pro and interfaces that speak its protocol, on a serial port.
    EnttecPro { port: String },
    /// Enttec Open DMX USB and its clones: an FTDI cable the computer times.
    OpenDmx { port: String },
    /// Art-Net to `host` (an address, or a broadcast address), at a port-address.
    ArtNet { host: String, universe: u16 },
    /// sACN (E1.31) to a universe, by multicast, or to `host` when given.
    Sacn { universe: u16, host: Option<String> },
    /// SoundSwitch Micro DMX Interface (experimental: its protocol is not published).
    SoundSwitch,
}

/// Where OSC messages go, over UDP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OscTarget {
    pub host: String,
    pub port: u16,
}

/// Which MIDI output gets the notes, by its name, on which channel (1 to 16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MidiTarget {
    pub port: String,
    pub channel: u8,
}

/// Where the show's events are sent (`signals`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct MessageSetup {
    pub osc: Option<OscTarget>,
    pub midi: Option<MidiTarget>,
}

/// The venue's lights.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct LightsSetup {
    pub output: OutputChoice,
    pub fixtures: Vec<Fixture>,
    pub looks: Vec<Look>,
    /// The look each stage of the show takes. A stage with none keeps the
    /// look before it.
    pub stages: BTreeMap<Stage, LookId>,
    /// The look a cue flashes, and for how long.
    pub flashes: BTreeMap<Cue, Flash>,
    /// Every look's intensity is scaled by this, in percent.
    pub master: u8,
    pub messages: MessageSetup,
}

/// Why a setup was refused (`LightsSetup::check`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "problem", rename_all = "camelCase")]
pub enum SetupError {
    #[error("too many {what}")]
    TooMany { what: Part },
    #[error("a {what} has a value out of range")]
    OutOfRange { what: Part },
    #[error("two {what}s have the same id")]
    SameId { what: Part },
    #[error("a look or flash names a look or fixture that is not there")]
    Missing,
    #[error("fixture \"{name}\" goes past channel 512")]
    PastUniverse { name: String },
    /// Network outputs and OSC take an IPv4 address, never a name: looking
    /// a name up can take seconds, and the lights thread would stop meanwhile.
    #[error("\"{host}\" is no IPv4 address")]
    NotIpv4 { host: String },
}

/// The part of a setup a `SetupError` is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Part {
    Fixture,
    Channel,
    Look,
    Color,
    Flash,
    Output,
    Message,
    Master,
}

impl std::fmt::Display for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Fails with `error` unless `ok`.
fn ensure(ok: bool, error: SetupError) -> Result<(), SetupError> {
    if ok {
        Ok(())
    } else {
        Err(error)
    }
}

fn out_of_range(what: Part) -> SetupError {
    SetupError::OutOfRange { what }
}

fn name_fits(name: &str) -> bool {
    name.chars().count() <= MAX_LIGHTS_NAME_CHARS
}

/// `host` as an IPv4 address, as network outputs and OSC take it.
pub fn ipv4(host: &str) -> Option<Ipv4Addr> {
    host.trim().parse().ok()
}

fn check_ipv4(host: &str) -> Result<(), SetupError> {
    ensure(ipv4(host).is_some(), SetupError::NotIpv4 { host: host.chars().take(MAX_HOST_CHARS).collect() })
}

fn port_fits(port: &str) -> bool {
    !port.is_empty() && port.len() <= MAX_PORT_CHARS
}

/// Whether `address` is a channel of the universe.
fn in_universe(address: usize) -> bool {
    (1..=UNIVERSE_SIZE).contains(&address)
}

impl Default for LightsSetup {
    /// No output and no fixtures yet, with looks for every stage and cue,
    /// so the lights follow the show as soon as fixtures are added.
    fn default() -> LightsSetup {
        let look = |id: LookId, name: &str, colors: &[&str], intensity: u8, effect: LookEffect, step_ms: u32, fade_ms: u32| Look {
            id,
            name: name.to_owned(),
            colors: colors.iter().map(|hex| Color::try_from((*hex).to_owned()).expect("built-in colours are hex")).collect(),
            intensity,
            effect,
            step_ms,
            fade_ms,
            fixtures: Vec::new(),
            strobe: 0,
            channels: Vec::new(),
        };
        let looks = vec![
            look(1, "Welcome", &["#FF9A3C", "#FFD08A"], 60, LookEffect::Static, 1_000, 1_500),
            look(2, "Question", &["#2F6BFF"], 70, LookEffect::Static, 1_000, 800),
            look(3, "Answering", &["#2F6BFF", "#7A3CFF"], 100, LookEffect::Chase, 1_000, 500),
            look(4, "Last seconds", &["#FF3B30"], 100, LookEffect::Pulse, 500, 200),
            look(5, "Buzz", &["#FFFFFF"], 100, LookEffect::Static, 1_000, 0),
            look(6, "Correct", &["#2ECC40"], 100, LookEffect::Static, 1_000, 0),
            look(7, "Wrong", &["#FF2020"], 100, LookEffect::Static, 1_000, 0),
            look(8, "Reveal", &["#B455FF"], 90, LookEffect::Static, 1_000, 300),
            look(9, "Leaderboard", &["#FFC000", "#FF7A00"], 90, LookEffect::Chase, 600, 800),
            look(10, "Drum roll", &["#FFFFFF", "#2F6BFF"], 100, LookEffect::Pulse, 600, 0),
            look(11, "Winner", &["#FFC000", "#FF3B30", "#2ECC40", "#2F6BFF", "#B455FF"], 100, LookEffect::Chase, 500, 0),
            look(12, "Dim", &["#FFD08A"], 25, LookEffect::Static, 1_000, 2_000),
        ];
        let stages = BTreeMap::from([
            (Stage::NoShow, 12),
            (Stage::Lobby, 1),
            (Stage::Billboard, 1),
            (Stage::Explanation, 2),
            (Stage::EndOfRound, 1),
            (Stage::QuestionReady, 2),
            (Stage::Answering, 3),
            (Stage::LastSeconds, 4),
            (Stage::Buzzed, 5),
            (Stage::Closed, 2),
            (Stage::Revealed, 8),
            (Stage::Leaderboard, 9),
            (Stage::FinalBuildUp, 10),
            (Stage::Winner, 11),
        ]);
        let flashes = BTreeMap::from([(Cue::Correct, Flash { look: 6, ms: 1_500 }), (Cue::Wrong, Flash { look: 7, ms: 1_500 })]);
        LightsSetup { output: OutputChoice::None, fixtures: Vec::new(), looks, stages, flashes, master: 100, messages: MessageSetup::default() }
    }
}

impl LightsSetup {
    pub fn look(&self, id: LookId) -> Option<&Look> {
        self.looks.iter().find(|look| look.id == id)
    }

    /// Refuses a setup that does not fit: too many parts, values out of
    /// range, two parts with one id, a name of a look or fixture that is
    /// not there, or a fixture past the universe. The checks of what comes
    /// in from a window or a file.
    pub fn check(&self) -> Result<(), SetupError> {
        self.check_fixtures()?;
        self.check_looks()?;
        let looks: BTreeSet<LookId> = self.looks.iter().map(|look| look.id).collect();
        ensure(self.stages.values().all(|id| looks.contains(id)), SetupError::Missing)?;
        for flash in self.flashes.values() {
            ensure(looks.contains(&flash.look), SetupError::Missing)?;
            ensure((MIN_FLASH_MS..=MAX_FLASH_MS).contains(&flash.ms), out_of_range(Part::Flash))?;
        }
        ensure(self.master <= 100, out_of_range(Part::Master))?;
        check_output(&self.output)?;
        check_messages(&self.messages)
    }

    fn check_fixtures(&self) -> Result<(), SetupError> {
        ensure(self.fixtures.len() <= MAX_FIXTURES, SetupError::TooMany { what: Part::Fixture })?;
        let mut ids = BTreeSet::new();
        for fixture in &self.fixtures {
            ensure(ids.insert(fixture.id), SetupError::SameId { what: Part::Fixture })?;
            ensure(name_fits(&fixture.name), out_of_range(Part::Fixture))?;
            ensure(!fixture.channels.is_empty(), out_of_range(Part::Channel))?;
            ensure(fixture.channels.len() <= MAX_FIXTURE_CHANNELS, SetupError::TooMany { what: Part::Channel })?;
            ensure(fixture.labels.len() <= fixture.channels.len(), SetupError::TooMany { what: Part::Channel })?;
            ensure(fixture.labels.iter().all(|label| label.chars().count() <= MAX_LIGHTS_NAME_CHARS), out_of_range(Part::Channel))?;
            ensure(in_universe(usize::from(fixture.address)), out_of_range(Part::Fixture))?;
            ensure(fixture.last_address() <= UNIVERSE_SIZE, SetupError::PastUniverse { name: fixture.name.clone() })?;
        }
        Ok(())
    }

    fn check_looks(&self) -> Result<(), SetupError> {
        ensure(self.looks.len() <= MAX_LOOKS, SetupError::TooMany { what: Part::Look })?;
        let fixtures: BTreeSet<FixtureId> = self.fixtures.iter().map(|f| f.id).collect();
        let mut ids = BTreeSet::new();
        for look in &self.looks {
            ensure(ids.insert(look.id), SetupError::SameId { what: Part::Look })?;
            ensure(name_fits(&look.name), out_of_range(Part::Look))?;
            ensure(!look.colors.is_empty(), out_of_range(Part::Color))?;
            ensure(look.colors.len() <= MAX_LOOK_COLORS, SetupError::TooMany { what: Part::Color })?;
            ensure(look.intensity <= 100, out_of_range(Part::Look))?;
            ensure((MIN_EFFECT_MS..=MAX_EFFECT_MS).contains(&look.step_ms), out_of_range(Part::Look))?;
            ensure(look.fade_ms <= MAX_FADE_MS, out_of_range(Part::Look))?;
            ensure(look.fixtures.iter().all(|id| fixtures.contains(id)), SetupError::Missing)?;
            ensure(look.channels.len() <= MAX_LOOK_CHANNELS, SetupError::TooMany { what: Part::Channel })?;
            ensure(look.channels.iter().all(|c| in_universe(usize::from(c.address))), out_of_range(Part::Channel))?;
        }
        Ok(())
    }
}

fn check_output(output: &OutputChoice) -> Result<(), SetupError> {
    let fits = match output {
        OutputChoice::None | OutputChoice::SoundSwitch => true,
        OutputChoice::EnttecPro { port } | OutputChoice::OpenDmx { port } => port_fits(port),
        OutputChoice::ArtNet { host, universe } => {
            check_ipv4(host)?;
            *universe <= MAX_ARTNET_UNIVERSE
        }
        OutputChoice::Sacn { universe, host } => {
            if let Some(host) = host {
                check_ipv4(host)?;
            }
            (MIN_SACN_UNIVERSE..=MAX_SACN_UNIVERSE).contains(universe)
        }
    };
    ensure(fits, out_of_range(Part::Output))
}

fn check_messages(messages: &MessageSetup) -> Result<(), SetupError> {
    if let Some(osc) = &messages.osc {
        check_ipv4(&osc.host)?;
        ensure(osc.port != 0, out_of_range(Part::Message))?;
    }
    if let Some(midi) = &messages.midi {
        ensure(port_fits(&midi.port) && (1..=16).contains(&midi.channel), out_of_range(Part::Message))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn par(id: FixtureId, address: u16) -> Fixture {
        Fixture { id, name: format!("Par {id}"), address, channels: vec![Channel::Dimmer, Channel::Red, Channel::Green, Channel::Blue], labels: Vec::new() }
    }

    #[test]
    fn the_default_setup_has_a_look_for_every_stage_and_passes_its_checks() {
        let setup = LightsSetup::default();
        assert_eq!(setup.check(), Ok(()));
        for stage in Stage::ALL {
            assert!(setup.stages.contains_key(&stage), "{stage:?}");
        }
    }

    #[test]
    fn a_setup_reads_back_as_written_and_a_missing_part_takes_its_default() {
        let mut setup = LightsSetup { fixtures: vec![par(1, 1)], ..LightsSetup::default() };
        setup.output = OutputChoice::Sacn { universe: 2, host: None };
        let json = serde_json::to_string(&setup).unwrap();
        assert_eq!(serde_json::from_str::<LightsSetup>(&json).unwrap(), setup);
        let partial: LightsSetup = serde_json::from_str(r#"{"master":50}"#).unwrap();
        assert_eq!(partial, LightsSetup { master: 50, ..LightsSetup::default() });
    }

    #[test]
    fn a_fixture_must_fit_in_the_universe() {
        let setup = |address| LightsSetup { fixtures: vec![par(1, address)], ..LightsSetup::default() };
        assert_eq!(setup(509).check(), Ok(()));
        assert_eq!(setup(510).check(), Err(SetupError::PastUniverse { name: "Par 1".into() }));
        assert_eq!(setup(0).check(), Err(SetupError::OutOfRange { what: Part::Fixture }));
    }

    #[test]
    fn looks_and_flashes_must_name_what_is_there() {
        let mut setup = LightsSetup::default();
        setup.stages.insert(Stage::Lobby, 99);
        assert_eq!(setup.check(), Err(SetupError::Missing));
        let mut setup = LightsSetup::default();
        setup.looks[0].fixtures = vec![5];
        assert_eq!(setup.check(), Err(SetupError::Missing));
        let mut setup = LightsSetup::default();
        setup.flashes.insert(Cue::Buzz, Flash { look: 5, ms: 50 });
        assert_eq!(setup.check(), Err(SetupError::OutOfRange { what: Part::Flash }));
    }

    #[test]
    fn two_parts_with_one_id_are_refused() {
        let setup = LightsSetup { fixtures: vec![par(1, 1), par(1, 10)], ..LightsSetup::default() };
        assert_eq!(setup.check(), Err(SetupError::SameId { what: Part::Fixture }));
        let mut setup = LightsSetup::default();
        setup.looks[1].id = setup.looks[0].id;
        assert_eq!(setup.check(), Err(SetupError::SameId { what: Part::Look }));
    }

    #[test]
    fn outputs_and_messages_must_be_in_range() {
        let refused = |output: OutputChoice| LightsSetup { output, ..LightsSetup::default() }.check();
        assert!(refused(OutputChoice::ArtNet { host: "2.255.255.255".into(), universe: 0 }).is_ok());
        assert!(refused(OutputChoice::ArtNet { host: " ".into(), universe: 0 }).is_err());
        assert!(refused(OutputChoice::ArtNet { host: "10.0.0.5".into(), universe: MAX_ARTNET_UNIVERSE + 1 }).is_err());
        assert!(refused(OutputChoice::Sacn { universe: 0, host: None }).is_err());
        assert!(refused(OutputChoice::EnttecPro { port: String::new() }).is_err());
        let mut setup = LightsSetup::default();
        setup.messages.midi = Some(MidiTarget { port: "loopMIDI".into(), channel: 17 });
        assert_eq!(setup.check(), Err(SetupError::OutOfRange { what: Part::Message }));
    }

    #[test]
    fn network_outputs_and_osc_take_only_an_ipv4_address() {
        let refused = |output: OutputChoice| LightsSetup { output, ..LightsSetup::default() }.check();
        let not_ipv4 = |host: &str| Err(SetupError::NotIpv4 { host: host.into() });
        assert_eq!(refused(OutputChoice::ArtNet { host: "lights.local".into(), universe: 0 }), not_ipv4("lights.local"));
        assert_eq!(refused(OutputChoice::Sacn { universe: 1, host: Some("::1".into()) }), not_ipv4("::1"));
        assert!(refused(OutputChoice::Sacn { universe: 1, host: Some(" 10.0.0.9 ".into()) }).is_ok());
        let mut setup = LightsSetup::default();
        setup.messages.osc = Some(OscTarget { host: "localhost".into(), port: 7700 });
        assert_eq!(setup.check(), not_ipv4("localhost"));
    }

    #[test]
    fn no_effect_flashes_more_than_twice_a_second() {
        let mut setup = LightsSetup::default();
        assert!(setup.looks.iter().all(|look| look.step_ms >= MIN_EFFECT_MS));
        setup.looks[0].effect = LookEffect::Strobe;
        setup.looks[0].step_ms = MIN_EFFECT_MS - 1;
        assert_eq!(setup.check(), Err(SetupError::OutOfRange { what: Part::Look }));
    }
}
