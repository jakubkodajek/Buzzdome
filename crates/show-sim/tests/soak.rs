//! Random shows played to the end, checking every rule after every step
//! (`show_sim::Show::check`). `SOAK_SHOWS` sets how many (40 by default;
//! the nightly run plays thousands, in release), `SOAK_FIRST` the first
//! seed, and `SOAK_SEED` replays one show that failed. The long evening is
//! left to the nightly run (`--include-ignored`), so every push stays quick.

use show_sim::{play, play_many, Plan, MAX_BUZZERS};

fn env(name: &str) -> Option<u64> {
    std::env::var(name).ok().map(|value| value.parse().unwrap_or_else(|_| panic!("{name} must be a number, not {value:?}")))
}

#[test]
fn random_shows_keep_every_rule() {
    if let Some(seed) = env("SOAK_SEED") {
        play(seed, Plan::default());
        return;
    }
    play_many(env("SOAK_FIRST").unwrap_or(0), env("SOAK_SHOWS").unwrap_or(40), Plan::default());
}

#[test]
#[ignore = "takes about 20 s in debug; the nightly run plays it"]
fn a_long_evening_with_fifty_buzzers_keeps_every_rule() {
    // One show of thousands of steps: a whole evening's journal, replayed in full now and then.
    let show = play(env("SOAK_SEED").unwrap_or(2026), Plan { steps: 3_000, replay_every: 500 });
    assert!(show.journal.len() > 1_000, "the show took only {} commands", show.journal.len());
    assert!(show.buzzers <= MAX_BUZZERS);
}
