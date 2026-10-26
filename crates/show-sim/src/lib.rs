//! Simulated shows for tests. A seeded generator writes random quizzes and
//! plays them as a quizmaster, a show thread and up to `MAX_BUZZERS` buzzers
//! would, and `Show::step` checks after every command the rules a live
//! event depends on (`Show::check`). A failure names its seed, so
//! `SOAK_SEED=<seed>` replays exactly that show.
//!
//! The generator gives the core what the app can give it: the quizmaster's
//! buttons (mostly the ones `Game::controls` offers, sometimes any), presses
//! from any buzzer in bursts and late, time up from the show thread, team
//! edits with odd names, score changes, and crashes followed by a resume.

use quiz_core::{
    AllAnswered, AnswerMode, Button, BuzzerMode, Command, CorrectAnswer, Effect, Game, GameError, Light, Phase, Question, QuestionStep, Quiz,
    QuizSettings, Slide, SlideContent, Stamped, VideoClip, FULL_SHARE, MAX_ALL_ANSWERED_DELAY_S, MAX_ANSWERS, MAX_EVERYONE_ANSWERS, MAX_POINTS,
    MAX_TEAMS, MIN_EVERYONE_ANSWERS, RULES_VERSION,
};
use std::collections::BTreeMap;

/// The most buzzers a show is planned for (specification, BZ-1).
pub const MAX_BUZZERS: u16 = 50;

/// A small seeded generator (SplitMix64), so a failing show replays from its seed.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `range` (inclusive).
    pub fn between(&mut self, low: u64, high: u64) -> u64 {
        low + self.next_u64() % (high - low + 1)
    }

    /// An index below `len` (which must not be 0).
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len as u64) as usize
    }

    /// True `percent` times in a hundred.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.next_u64() % 100 < percent
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }
}

const BUTTONS: [Button; 5] = [Button::A, Button::B, Button::C, Button::D, Button::Big];

/// Names a quizmaster may type or paste: Czech letters, spaces around,
/// case twins, blanks, and ones past `MAX_NAME_CHARS`.
fn random_name(rng: &mut Rng) -> String {
    const NAMES: [&str; 10] = ["Žluťoučký kůň", "žluťoučký KŮŇ", "Tým 1", "  Sovy  ", "", "   ", "Ostrava", "ŘEŘICHA", "a", "Ďábelské ůčko"];
    match rng.between(0, 9) {
        0 => "Příliš dlouhé jméno ".repeat(rng.between(3, 8) as usize),
        1 => format!("Team {}", rng.between(1, 60)),
        _ => rng.pick(&NAMES).to_string(),
    }
}

/// A random quiz the editor could save: every slide type, both answer
/// modes, 0 to `MAX_ANSWERS` answers, part shares, untimed and timed
/// clocks, rising and falling points, videos, a final slide or none, and
/// every step the show can take once everyone answered.
pub fn random_quiz(rng: &mut Rng) -> Quiz {
    let count = rng.between(1, 14) as usize;
    let mut slides = Vec::with_capacity(count + 1);
    for n in 0..count {
        let id = format!("s{n}");
        let content = match rng.between(0, 9) {
            0 => SlideContent::Billboard { title: format!("Kolo {n}"), text: "Text".into() },
            1 => SlideContent::Explanation { text: "Vysvětlení".into() },
            2 => SlideContent::EndOfRound { title: "Konec kola".into() },
            3 if rng.chance(30) => SlideContent::Final { title: "Finále".into() },
            _ => SlideContent::Question(random_question(rng, n)),
        };
        let mut slide = Slide::new(id, content);
        let plays_video = matches!(slide.content, SlideContent::Billboard { .. } | SlideContent::Explanation { .. } | SlideContent::Question(_));
        if plays_video && rng.chance(15) {
            slide.video = Some(serde_json::from_value::<VideoClip>(serde_json::json!({ "file": "media/clip.mp4", "autoplay": rng.chance(50) })).unwrap());
        }
        slides.push(slide);
    }
    if rng.chance(25) {
        slides.push(Slide::new("final", SlideContent::Final { title: "Výsledky".into() }));
    }
    let mut quiz = Quiz::new("Simulace", slides);
    quiz.settings = QuizSettings {
        all_answered: *rng.pick(&[AllAnswered::Wait, AllAnswered::Reveal, AllAnswered::NextSlide]),
        all_answered_delay_s: rng.between(0, u64::from(MAX_ALL_ANSWERED_DELAY_S)) as u8,
    };
    if let Err(errors) = quiz.validate() {
        panic!("the generator wrote a quiz the editor would refuse: {errors:?}");
    }
    quiz
}

fn random_question(rng: &mut Rng, n: usize) -> Question {
    let mode = if rng.chance(50) { AnswerMode::Everyone } else { AnswerMode::FastestFinger };
    let answer_count = match mode {
        AnswerMode::Everyone => rng.between(MIN_EVERYONE_ANSWERS as u64, MAX_EVERYONE_ANSWERS as u64) as usize,
        AnswerMode::FastestFinger => rng.between(0, MAX_ANSWERS as u64) as usize,
    };
    let answers: Vec<String> = (0..answer_count).map(|a| format!("Odpověď {a}")).collect();
    let mut correct = Vec::new();
    for answer in 0..answer_count {
        let first = answer == 0 && mode == AnswerMode::Everyone;
        if first || rng.chance(25) {
            let share = if mode == AnswerMode::Everyone && rng.chance(40) { rng.between(1, u64::from(FULL_SHARE)) as u8 } else { FULL_SHARE };
            correct.push(CorrectAnswer { answer, share });
        }
    }
    let points = match rng.between(0, 9) {
        0 => MAX_POINTS,
        1 => -(rng.between(0, 500) as i32),
        _ => rng.between(0, 500) as i32,
    };
    Question {
        text: format!("Otázka {n}?"),
        mode,
        answers,
        correct,
        time_limit_s: match rng.between(0, 9) {
            0 => 0,
            1 => 1,
            _ => rng.between(2, 40) as u32,
        },
        points,
        points_end: rng.chance(40).then(|| rng.between(0, 300) as i32),
        penalty: if rng.chance(50) { rng.between(0, 100) as i32 } else { 0 },
    }
}

/// The step commands the Director's buttons give.
fn step_commands(game: &Game, rng: &mut Rng) -> Vec<Command> {
    let slides = game.quiz().slides.len();
    vec![
        Command::Next,
        Command::JudgeRight,
        Command::JudgeWrong,
        Command::PauseClock,
        Command::ResumeClock,
        Command::CloseQuestion,
        Command::RestartQuestion,
        Command::PlayVideo,
        Command::GoToFinal,
        Command::GoTo { slide: rng.index(slides + 2) },
    ]
}

/// How often a quizmaster presses a step button the Director offers:
/// mostly Next and the verdicts, now and then a jump.
fn quizmaster_weight(command: &Command) -> u64 {
    match command {
        Command::Next => 50,
        Command::JudgeRight | Command::JudgeWrong => 20,
        Command::ResumeClock => 12,
        Command::PauseClock => 8,
        Command::CloseQuestion => 6,
        Command::RestartQuestion | Command::PlayVideo => 4,
        Command::GoTo { .. } => 3,
        _ => 1,
    }
}

/// Whether `Game::controls` offers `command` now (`None`: not a step command).
fn offered(game: &Game, command: &Command) -> Option<bool> {
    let c = game.controls();
    Some(match command {
        Command::Next => c.next,
        Command::JudgeRight | Command::JudgeWrong => c.judge,
        Command::PauseClock => c.pause_clock,
        Command::ResumeClock => c.resume_clock,
        Command::CloseQuestion => c.close_question,
        Command::RestartQuestion => c.restart_question,
        Command::PlayVideo => c.play_video,
        Command::GoToFinal => c.go_to,
        Command::GoTo { slide } => c.go_to && *slide < game.quiz().slides.len(),
        _ => return None,
    })
}

/// How hard a show is played.
#[derive(Debug, Clone, Copy)]
pub struct Plan {
    /// Commands and presses per show.
    pub steps: usize,
    /// Replay the whole journal and compare every this many steps (it costs
    /// the journal's length each time).
    pub replay_every: usize,
}

impl Default for Plan {
    fn default() -> Plan {
        Plan { steps: 400, replay_every: 25 }
    }
}

/// One simulated show: the game, its journal as the app keeps it (only
/// commands the game took), and what `check` compares against.
#[derive(Debug, Clone)]
pub struct Show {
    pub game: Game,
    pub journal: Vec<Stamped>,
    /// The show clock, in milliseconds; it never goes back within a run.
    pub now: u64,
    pub buzzers: u16,
    /// Each player's score when the current question was entered, plus the
    /// quizmaster's own changes since (`AdjustScore`, which are not the
    /// question's), in a wider type so the sum is exact. None once a score
    /// came near the ends of `i32` in this question, where adding saturates
    /// (by design).
    baseline: Option<BTreeMap<u16, i64>>,
}

/// A command the show took or refused, for `check` and for the caller.
pub struct Stepped {
    pub stamped: Stamped,
    pub result: Result<Vec<Effect>, GameError>,
}

impl Show {
    pub fn new(quiz: Quiz, buzzers: u16) -> Show {
        Show { game: Game::new(quiz), journal: Vec::new(), now: 1_000, buzzers, baseline: Some(BTreeMap::new()) }
    }

    /// Applies `command` at the current time as the app does: a press that
    /// changes nothing is neither applied nor journaled (`Game::ignores`),
    /// anything else is applied and journaled if the game takes it. Checks
    /// every rule (`check`) on the way.
    pub fn apply(&mut self, command: Command) -> Stepped {
        let stamped = Stamped { at_ms: self.now, command };
        let before = self.game.clone();
        let ignored = before.ignores(&stamped);
        let result = if ignored { Ok(vec![]) } else { self.game.apply(&stamped) };
        if result.is_ok() && !ignored {
            self.track_baseline(&before, &stamped.command);
            self.journal.push(stamped.clone());
        }
        self.check(&before, &stamped, ignored, &result);
        Stepped { stamped, result }
    }

    /// What the show thread does on its tick: time up, or the step the
    /// quiz's settings take once everyone answered, if due.
    pub fn tick(&mut self) {
        if let Some(command) = self.game.due_command(self.now) {
            let stepped = self.apply(command);
            assert!(stepped.result.is_ok(), "a due command was refused: {:?}", stepped.result);
        }
    }

    /// The show as the app rebuilds it after a crash: replayed from the
    /// journal, with a question that was in play restarted and a step
    /// coming by itself held (journaled too).
    pub fn crash_and_resume(&mut self) {
        let replayed = Game::replay(self.game.quiz().clone(), &self.journal, RULES_VERSION);
        assert_eq!(replayed, self.game, "the journal does not rebuild the show the windows saw");
        // A restarted run's clock may read earlier than the last command (the system clock went back).
        if self.journal.last().is_some() && self.now > 5_000 && self.now.is_multiple_of(3) {
            self.now -= 3_000;
        }
        if let Some(command) = self.game.recovery_command() {
            let stepped = self.apply(command);
            assert!(stepped.result.is_ok(), "the recovery command was refused: {:?}", stepped.result);
            assert!(!self.game.controls().restart_question, "a question is still in play after the recovery");
        }
        assert_eq!(self.game.view(self.now).auto_step, None, "a step comes by itself right after the recovery");
    }

    /// Asserts that the whole journal rebuilds the current game.
    pub fn assert_replays(&self) {
        let replayed = Game::replay(self.game.quiz().clone(), &self.journal, RULES_VERSION);
        assert_eq!(replayed, self.game, "replaying the journal gave another show");
    }

    /// The rules every step keeps.
    fn check(&self, before: &Game, stamped: &Stamped, ignored: bool, result: &Result<Vec<Effect>, GameError>) {
        let after = &self.game;
        if ignored || result.is_err() {
            assert_eq!(after, before, "an ignored or refused command changed the show: {stamped:?} -> {result:?}");
        }
        if let Command::Press { .. } = stamped.command {
            assert!(result.is_ok(), "a press must never fail: {result:?}");
            if !ignored {
                assert_ne!(after, before, "a press that was not ignored changed nothing: {stamped:?}");
            }
        }
        if let Some(offered) = offered(before, &stamped.command) {
            assert_eq!(offered, result.is_ok(), "controls offered {offered} but apply gave {result:?} for {:?} in {:?}", stamped.command, before.phase());
        }
        if let Ok(effects) = result {
            for effect in effects {
                if let Effect::Lights { lights } = effect {
                    for (buzzer, _) in lights {
                        assert!(after.players().any(|p| p.buzzer == *buzzer), "a light for buzzer {buzzer}, which is not in the show");
                    }
                }
            }
        }
        // Every buzzer ready in a fastest-finger round that begins showed
        // another light just before: the buzzes are ordered by the buzzers'
        // timers, and an Enjoy 303S buzzer's starts again only when its
        // colour changes (docs/design.md, 2.6).
        if after.buzzer_mode() == BuzzerMode::FastestFinger && before.buzzer_mode() != BuzzerMode::FastestFinger {
            let was: BTreeMap<_, _> = before.lights().into_iter().collect();
            for (buzzer, light) in after.lights() {
                assert!(light != Light::Ready || was.get(&buzzer) != Some(&Light::Ready), "buzzer {buzzer} was ready already as a round began: {stamped:?}");
            }
        }
        self.check_view(after);
        self.check_scores(after, stamped);
    }

    /// What the windows get must describe the game consistently, and serialize.
    fn check_view(&self, game: &Game) {
        let view = game.view(self.now);
        serde_json::to_string(&view).expect("the view does not serialize");
        assert_eq!(view.controls, game.controls());
        let enabled = game.players().filter(|p| p.enabled).count();
        assert_eq!(view.enabled_count, enabled);
        assert!(view.answered_count <= view.enabled_count, "{} answered of {} enabled", view.answered_count, view.enabled_count);
        let step = match game.phase() {
            Phase::Question(step) => Some(step),
            _ => None,
        };
        let over = matches!(step, Some(QuestionStep::Closed | QuestionStep::Revealed { .. }));
        if view.answer_counts.is_some() || view.results.is_some() {
            assert!(over, "answer counts or results before the question closed: {:?}", game.phase());
        }
        match step {
            Some(QuestionStep::Buzzed { buzzer, .. }) => assert_eq!(view.buzzed, Some(*buzzer)),
            _ => assert_eq!(view.buzzed, None),
        }
        for player in &view.players {
            assert_eq!(player.rank.is_some(), player.enabled, "rank of buzzer {}", player.buzzer);
        }
        // The view's places are the game's (`Game::ranking`), best first: the
        // first is 1, a player shares the place above or takes its own in the
        // order (1, 1, 3), and a better place never has a lower score.
        let ranking = game.ranking();
        assert_eq!(ranking.len(), enabled);
        for (i, &(place, player)) in ranking.iter().enumerate() {
            let rank = view.players.iter().find(|p| p.buzzer == player.buzzer).and_then(|p| p.rank);
            assert_eq!(rank, Some(place), "rank of buzzer {}", player.buzzer);
            let Some(&(above_place, above)) = i.checked_sub(1).and_then(|above| ranking.get(above)) else {
                assert_eq!(place, 1, "the best place is not 1");
                continue;
            };
            assert!(place == above_place || place == i + 1, "place {place} after place {above_place} as number {}", i + 1);
            assert!(above.score >= player.score, "place {above_place} scores {} and place {place} scores {}", above.score, player.score);
            assert!(place != above_place || above.score == player.score, "a shared place {place} with scores {} and {}", above.score, player.score);
        }
        if let Some(clock) = game.clock_view(self.now) {
            if let (Some(limit), Some(remaining)) = (clock.limit_ms, clock.remaining_ms) {
                assert!(remaining <= limit && clock.elapsed_ms <= limit);
            }
        }
        if let Some(slide) = game.current_slide() {
            assert!(game.quiz().slides.iter().any(|s| s.id == slide.id));
        }
    }

    /// While a question is on screen, every point a player won or lost in it
    /// is in its score changes (`Game::delta`), which a restart takes back.
    fn check_scores(&self, game: &Game, stamped: &Stamped) {
        let Some(baselines) = &self.baseline else { return };
        if !matches!(game.phase(), Phase::Question(_)) {
            return;
        }
        for player in game.players() {
            let baseline = baselines.get(&player.buzzer).copied().unwrap_or(0);
            let delta = i64::from(game.delta(player.buzzer).unwrap_or(0));
            assert_eq!(
                i64::from(player.score),
                baseline + delta,
                "buzzer {}: score {} is not its score before the question ({baseline}) and the question's change ({delta}) after {stamped:?}",
                player.buzzer,
                player.score
            );
        }
    }

    /// Keeps `baseline` after `command` changed the show from `before`.
    fn track_baseline(&mut self, before: &Game, command: &Command) {
        let entered = !matches!(before.phase(), Phase::Question(QuestionStep::Ready)) || before.slide_index() != self.game.slide_index();
        let fresh = matches!(self.game.phase(), Phase::Question(QuestionStep::Ready)) && (entered || matches!(command, Command::RestartQuestion));
        if fresh {
            self.baseline = Some(self.game.players().map(|p| (p.buzzer, i64::from(p.score))).collect());
        } else if let (Some(baselines), Command::AdjustScore { buzzer, delta }) = (&mut self.baseline, command) {
            *baselines.entry(*buzzer).or_default() += i64::from(*delta);
        }
        if near_the_limits(&self.game) {
            self.baseline = None;
        }
    }

    /// The next thing that happens in the show, as the app would give it.
    pub fn random_command(&mut self, rng: &mut Rng) -> Command {
        let game = &self.game;
        match rng.between(0, 99) {
            // The quizmaster presses a button the Director shows.
            // At the end the quizmaster may go back to any slide (a replay, a rehearsal).
            0..=34 if show_over(game) => {
                Command::GoTo { slide: rng.index(game.quiz().slides.len()) }
            }
            0..=34 => {
                let offered: Vec<(Command, u64)> = step_commands(game, rng)
                    .into_iter()
                    .filter(|c| offered(game, c) == Some(true))
                    .map(|c| {
                        let weight = quizmaster_weight(&c);
                        (c, weight)
                    })
                    .collect();
                let total: u64 = offered.iter().map(|(_, weight)| weight).sum();
                let mut roll = rng.between(0, total.max(1) - 1);
                for (command, weight) in offered {
                    if roll < weight {
                        return command;
                    }
                    roll -= weight;
                }
                Command::Next
            }
            // A buzzer, mostly one of the show's, now and then any number.
            35..=69 => {
                let buzzer = if rng.chance(97) { rng.between(1, u64::from(self.buzzers)) as u16 } else { rng.next_u64() as u16 };
                Command::Press { buzzer, button: *rng.pick(&BUTTONS) }
            }
            // Any step command, offered or not (a key, a stale window).
            70..=79 => {
                let any = step_commands(game, rng);
                rng.pick(&any).clone()
            }
            // Teams and scores.
            80..=94 => {
                let buzzer = rng.between(1, u64::from(self.buzzers) + 2) as u16;
                match rng.between(0, 7) {
                    0 => Command::AddPlayer { buzzer, name: random_name(rng) },
                    1 => Command::RenamePlayer { buzzer, name: random_name(rng) },
                    2 => Command::PairTeam { buzzer, name: random_name(rng) },
                    3 => {
                        let count = if rng.chance(5) { MAX_TEAMS + 1 } else { rng.between(0, 12) as usize };
                        Command::SetTeams { names: (0..count).map(|n| if rng.chance(70) { format!("Team {n}") } else { random_name(rng) }).collect() }
                    }
                    4 => Command::SetPlayerEnabled { buzzer, enabled: rng.chance(60) },
                    _ => {
                        let delta = if rng.chance(3) { *rng.pick(&[i32::MAX, i32::MIN, MAX_POINTS * 10]) } else { rng.between(0, 200) as i32 - 100 };
                        Command::AdjustScore { buzzer, delta }
                    }
                }
            }
            _ => {
                if rng.chance(50) {
                    Command::ShowLeaderboard { visible: rng.chance(50) }
                } else {
                    Command::PlayVideo
                }
            }
        }
    }

    /// Moves the show clock on: mostly a moment, sometimes not at all (a
    /// burst in one millisecond), sometimes past any question's time limit.
    pub fn random_wait(&mut self, rng: &mut Rng) {
        self.now += match rng.between(0, 99) {
            0..=19 => 0,
            20..=89 => rng.between(1, 2_500),
            90..=97 => rng.between(2_500, 45_000),
            _ => rng.between(45_000, 4_000_000),
        };
    }

    /// Plays the show to its end as a quizmaster would: the question in play
    /// is ended, buzzes judged, and Next pressed until the show is over.
    /// Fails if that takes more steps than the quiz could need.
    pub fn finish(&mut self) {
        let bound = (self.game.quiz().slides.len() + 2) * 12 + 20;
        for _ in 0..bound {
            let controls = self.game.controls();
            let command = if controls.judge {
                Command::JudgeRight
            } else if controls.close_question {
                Command::CloseQuestion
            } else if controls.next {
                Command::Next
            } else {
                assert!(!controls.restart_question, "a question in play offers no way on: {:?}", self.game.phase());
                return;
            };
            self.now += 500;
            let stepped = self.apply(command);
            assert!(stepped.result.is_ok(), "an offered command was refused at the end: {:?}", stepped.result);
        }
        panic!("the show did not end within {bound} steps: {:?}", self.game.phase());
    }
}

/// Nothing is left to do but jump: the show is over.
fn show_over(game: &Game) -> bool {
    let c = game.controls();
    !(c.next || c.judge || c.restart_question)
}

/// A score near the ends of `i32`: adding may saturate there (by design),
/// so a sum of score changes no longer adds up exactly.
fn near_the_limits(game: &Game) -> bool {
    game.players().any(|p| p.score.unsigned_abs() > (i32::MAX as u32) / 4)
}

/// Plays one random show from `seed` under `plan`, checking every rule.
pub fn play(seed: u64, plan: Plan) -> Show {
    let mut rng = Rng::new(seed);
    let quiz = random_quiz(&mut rng);
    let buzzers = if rng.chance(20) { MAX_BUZZERS } else { rng.between(1, u64::from(MAX_BUZZERS)) as u16 };
    let mut show = Show::new(quiz, buzzers);
    for step in 1..=plan.steps {
        show.random_wait(&mut rng);
        // Time up comes on the show thread's tick, before or after the events of that moment.
        let tick_first = rng.chance(50);
        if tick_first {
            show.tick();
        }
        let burst = if rng.chance(10) { rng.between(2, u64::from(show.buzzers).max(2)) } else { 1 };
        for _ in 0..burst {
            let command = show.random_command(&mut rng);
            show.apply(command);
        }
        if !tick_first {
            show.tick();
        }
        if rng.chance(2) {
            show.crash_and_resume();
        }
        if step % plan.replay_every == 0 {
            show.assert_replays();
        }
    }
    show.assert_replays();
    show.finish();
    show.assert_replays();
    show
}

/// Runs `play` for every seed from `first` on, `count` of them, naming the
/// seed of a show that fails.
pub fn play_many(first: u64, count: u64, plan: Plan) {
    for seed in first..first + count {
        if let Err(panic) = std::panic::catch_unwind(|| play(seed, plan)) {
            eprintln!("simulated show failed; replay it with SOAK_SEED={seed}");
            std::panic::resume_unwind(panic);
        }
    }
}
