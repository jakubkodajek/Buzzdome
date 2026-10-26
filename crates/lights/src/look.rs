//! Draws a look on the fixtures at a moment, into a DMX universe.

use crate::dmx::Universe;
use crate::setup::{Channel, Fixture, LightsSetup, Look, LookEffect};

/// The universe with every fixture dark: only their fixed channels set.
pub fn dark(setup: &LightsSetup) -> Universe {
    let mut universe = Universe::default();
    for fixture in &setup.fixtures {
        light(&mut universe, fixture, [0, 0, 0], 0.0, 0);
    }
    universe
}

/// `look` on the fixtures of `setup`, `elapsed_ms` after it began, at
/// `master` percent. The fixtures it does not light are dark.
pub fn draw(setup: &LightsSetup, look: &Look, elapsed_ms: u64, master: u8) -> Universe {
    let mut universe = dark(setup);
    let lit = setup.fixtures.iter().filter(|f| look.fixtures.is_empty() || look.fixtures.contains(&f.id));
    let step = elapsed_ms / u64::from(look.step_ms.max(1));
    let phase = (elapsed_ms % u64::from(look.step_ms.max(1))) as f32 / look.step_ms.max(1) as f32;
    let level = f32::from(look.intensity.min(100)) / 100.0 * f32::from(master.min(100)) / 100.0 * effect_level(look.effect, phase);
    let colors = look.colors.len().max(1);
    for (i, fixture) in lit.enumerate() {
        let turn = match look.effect {
            LookEffect::Chase => i as u64 + step,
            LookEffect::Static | LookEffect::Pulse | LookEffect::Strobe => i as u64,
        };
        let color = look.colors.get((turn % colors as u64) as usize).map_or([255, 255, 255], |c| c.rgb());
        light(&mut universe, fixture, color, level, look.strobe);
    }
    for channel in &look.channels {
        universe.set(usize::from(channel.address), channel.value);
    }
    universe
}

/// How much of the look's intensity its effect lets through at `phase`,
/// the part of a step gone (0 to 1): a pulse starts full and dims to dark
/// half a step in; a strobe is on for the first half of each step.
fn effect_level(effect: LookEffect, phase: f32) -> f32 {
    match effect {
        LookEffect::Static | LookEffect::Chase => 1.0,
        LookEffect::Pulse => 0.5 + 0.5 * (std::f32::consts::TAU * phase).cos(),
        LookEffect::Strobe => {
            if phase < 0.5 {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// Sets `fixture`'s channels for `color` at `level` (0 to 1). A fixture with
/// a dimmer takes the level there and the colour at full; one without
/// takes the colour dimmed.
fn light(universe: &mut Universe, fixture: &Fixture, [red, green, blue]: [u8; 3], level: f32, strobe: u8) {
    let has_dimmer = fixture.channels.iter().any(|channel| matches!(channel, Channel::Dimmer | Channel::DimmerRange { .. }));
    let scaled = |value: u8| if has_dimmer && level > 0.0 { value } else { (f32::from(value) * level).round() as u8 };
    for (offset, channel) in fixture.channels.iter().enumerate() {
        let value = match *channel {
            Channel::Dimmer => (255.0 * level).round() as u8,
            Channel::DimmerRange { dark, bright } => (f32::from(dark) + (f32::from(bright) - f32::from(dark)) * level).round() as u8,
            Channel::Red => scaled(red),
            Channel::Green => scaled(green),
            Channel::Blue => scaled(blue),
            Channel::White => scaled(red.min(green).min(blue)),
            Channel::Strobe => strobe,
            Channel::Shutter { open, closed } => {
                if level > 0.0 {
                    open
                } else {
                    closed
                }
            }
            Channel::Fixed { value } => value,
            Channel::Unused => 0,
        };
        universe.set(usize::from(fixture.address) + offset, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::ChannelValue;
    use quiz_core::Color;

    fn fixture(id: u32, address: u16, channels: Vec<Channel>) -> Fixture {
        Fixture { id, name: format!("F{id}"), address, channels, labels: Vec::new() }
    }

    fn color(hex: &str) -> Color {
        Color::try_from(hex.to_owned()).unwrap()
    }

    fn look(colors: &[&str], effect: LookEffect) -> Look {
        Look {
            id: 1,
            name: "L".into(),
            colors: colors.iter().map(|c| color(c)).collect(),
            intensity: 100,
            effect,
            step_ms: 1_000,
            fade_ms: 0,
            fixtures: Vec::new(),
            strobe: 0,
            channels: Vec::new(),
        }
    }

    fn rgb_pars(count: u32) -> LightsSetup {
        let fixtures = (0..count).map(|i| fixture(i + 1, 1 + 3 * i as u16, vec![Channel::Red, Channel::Green, Channel::Blue])).collect();
        LightsSetup { fixtures, ..LightsSetup::default() }
    }

    fn rgb(universe: &Universe, address: usize) -> [u8; 3] {
        [universe.get(address), universe.get(address + 1), universe.get(address + 2)]
    }

    #[test]
    fn a_fixture_with_a_dimmer_gets_the_level_there_and_the_colour_at_full() {
        let setup = LightsSetup {
            fixtures: vec![
                fixture(1, 1, vec![Channel::Dimmer, Channel::Red, Channel::Green, Channel::Blue, Channel::White, Channel::Strobe, Channel::Fixed { value: 7 }]),
                fixture(2, 20, vec![Channel::Red, Channel::Green, Channel::Blue, Channel::White, Channel::Unused]),
            ],
            ..LightsSetup::default()
        };
        let mut warm = look(&["#FF8040"], LookEffect::Static);
        warm.intensity = 50;
        warm.strobe = 33;
        let u = draw(&setup, &warm, 0, 100);
        assert_eq!((1..=7).map(|a| u.get(a)).collect::<Vec<_>>(), [128, 255, 128, 64, 64, 33, 7]);
        assert_eq!((20..=24).map(|a| u.get(a)).collect::<Vec<_>>(), [128, 64, 32, 32, 0]);
    }

    #[test]
    fn the_master_scales_every_look() {
        let u = draw(&rgb_pars(1), &look(&["#FFFFFF"], LookEffect::Static), 0, 20);
        assert_eq!(rgb(&u, 1), [51, 51, 51]);
    }

    #[test]
    fn colours_go_to_the_fixtures_in_turn_and_a_chase_moves_them_on_each_step() {
        let setup = rgb_pars(3);
        let two = ["#FF0000", "#0000FF"];
        let u = draw(&setup, &look(&two, LookEffect::Static), 5_000, 100);
        assert_eq!([rgb(&u, 1), rgb(&u, 4), rgb(&u, 7)], [[255, 0, 0], [0, 0, 255], [255, 0, 0]]);
        let chase = look(&two, LookEffect::Chase);
        let first = draw(&setup, &chase, 999, 100);
        let second = draw(&setup, &chase, 1_000, 100);
        assert_eq!([rgb(&first, 1), rgb(&second, 1)], [[255, 0, 0], [0, 0, 255]]);
    }

    #[test]
    fn a_pulse_starts_full_and_a_strobe_flashes_half_of_each_step() {
        let setup = rgb_pars(1);
        let pulse = look(&["#FFFFFF"], LookEffect::Pulse);
        assert_eq!([rgb(&draw(&setup, &pulse, 0, 100), 1)[0], rgb(&draw(&setup, &pulse, 500, 100), 1)[0]], [255, 0]);
        let strobe = look(&["#FFFFFF"], LookEffect::Strobe);
        assert_eq!([rgb(&draw(&setup, &strobe, 1_100, 100), 1)[0], rgb(&draw(&setup, &strobe, 1_600, 100), 1)[0]], [255, 0]);
    }

    #[test]
    fn a_look_lights_only_its_fixtures_and_sets_its_channels_last() {
        let setup = rgb_pars(2);
        let mut only_second = look(&["#00FF00"], LookEffect::Static);
        only_second.fixtures = vec![2];
        only_second.channels = vec![ChannelValue { address: 1, value: 9 }, ChannelValue { address: 100, value: 200 }];
        let u = draw(&setup, &only_second, 0, 100);
        assert_eq!([rgb(&u, 1), rgb(&u, 4)], [[9, 0, 0], [0, 255, 0]]);
        assert_eq!(u.get(100), 200);
    }

    #[test]
    fn dark_keeps_only_the_fixed_channels() {
        let setup = LightsSetup { fixtures: vec![fixture(1, 10, vec![Channel::Dimmer, Channel::Fixed { value: 42 }, Channel::Red])], ..LightsSetup::default() };
        let u = dark(&setup);
        assert_eq!([u.get(10), u.get(11), u.get(12)], [0, 42, 0]);
    }

    #[test]
    fn a_shutter_opens_only_while_the_fixture_is_lit() {
        let shutter = Channel::Shutter { open: 8, closed: 2 };
        let setup = LightsSetup { fixtures: vec![fixture(1, 1, vec![Channel::Dimmer, shutter])], ..LightsSetup::default() };
        assert_eq!(dark(&setup).get(2), 2);
        let look = setup.looks[1].clone();
        assert_eq!(draw(&setup, &look, 0, 100).get(2), 8);
        assert_eq!(draw(&setup, &look, 0, 0).get(2), 2);
    }

    #[test]
    fn a_dimmer_over_a_range_dims_within_it_either_way_and_counts_as_the_fixtures_dimmer() {
        let up_to = Channel::DimmerRange { dark: 0, bright: 200 };
        let inverted = Channel::DimmerRange { dark: 255, bright: 0 };
        let setup = LightsSetup { fixtures: vec![fixture(1, 1, vec![up_to, Channel::Red, inverted])], ..LightsSetup::default() };
        assert_eq!([dark(&setup).get(1), dark(&setup).get(3)], [0, 255]);
        let mut red = look(&["#ff0000"], LookEffect::Static);
        let channels = |look: &Look| {
            let u = draw(&setup, look, 0, 100);
            [u.get(1), u.get(2), u.get(3)]
        };
        assert_eq!(channels(&red), [200, 255, 0]);
        red.intensity = 50;
        assert_eq!(channels(&red), [100, 255, 128]);
        red.intensity = 0;
        assert_eq!(channels(&red), [0, 0, 255]);
    }
}
