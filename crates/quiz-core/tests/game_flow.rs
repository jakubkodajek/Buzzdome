use quiz_core::{
    AnswerMode, Button, Command, Cue, Effect, Game, GameError, Light, Phase, Question, QuestionStep, Quiz, Slide,
    SlideContent, Stamped, PRESS_GRACE_MS,
};
use quiz_core::view::AnswerResults;

fn question(id: &str, mode: AnswerMode) -> Slide {
    Slide::new(
        id,
        SlideContent::Question(Question {
            text: format!("{id}?"),
            mode,
            answers: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            correct: Some(1),
            time_limit_s: 10,
            points: 100,
            points_end: Some(50),
            penalty: 20,
        }),
    )
}

fn quiz(slides: Vec<Slide>) -> Quiz {
    Quiz { schema_version: quiz_core::SCHEMA_VERSION, title: "Test".into(), slides }
}

/// Applies commands and keeps the journal, like the app does.
struct Show {
    game: Game,
    journal: Vec<Stamped>,
}

impl Show {
    fn new(slides: Vec<Slide>) -> Show {
        Show { game: Game::new(quiz(slides)), journal: Vec::new() }
    }

    fn run(&mut self, at_ms: u64, command: Command) -> Result<Vec<Effect>, GameError> {
        let stamped = Stamped { at_ms, command };
        let effects = self.game.apply(&stamped)?;
        self.journal.push(stamped);
        Ok(effects)
    }

    fn press(&mut self, at_ms: u64, buzzer: u16, button: Button) -> Vec<Effect> {
        self.run(at_ms, Command::Press { buzzer, button }).unwrap()
    }

    fn score(&self, buzzer: u16) -> i32 {
        self.game.players().find(|p| p.buzzer == buzzer).unwrap().score
    }
}

fn step(game: &Game) -> QuestionStep {
    match game.phase() {
        Phase::Question(step) => step.clone(),
        other => panic!("not in a question: {other:?}"),
    }
}

#[test]
fn lobby_registers_buzzers_on_any_press() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 3, Button::Big);
    show.press(0, 1, Button::A);
    show.press(0, 3, Button::C);
    let buzzers: Vec<u16> = show.game.players().map(|p| p.buzzer).collect();
    assert_eq!(buzzers, vec![1, 3]);
    assert_eq!(show.game.phase(), &Phase::Lobby);
}

#[test]
fn everyone_answers_scores_by_speed_and_closes_when_all_answered() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    show.run(1_000, Command::Next).unwrap(); // to q1, Ready
    show.run(2_000, Command::Next).unwrap(); // clock starts at 2.0 s

    show.press(2_000, 1, Button::B); // right at 0 s: 100 points
    show.press(7_000, 2, Button::B); // right at 5 s: 75 points
    show.press(7_500, 2, Button::A); // a second answer does not count
    assert_eq!(step(&show.game), QuestionStep::Running { clock: *show.game.clock().unwrap() });

    show.press(8_000, 3, Button::C); // wrong: -20, and everyone has answered
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!((show.score(1), show.score(2), show.score(3)), (100, 75, -20));

    let effects = show.run(9_000, Command::Next).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Revealed { winner: None });
    assert!(effects.contains(&Effect::Lights {
        lights: vec![(1, Light::Correct), (2, Light::Correct), (3, Light::Wrong)]
    }));

    show.run(10_000, Command::Next).unwrap();
    assert_eq!(show.game.phase(), &Phase::Final { step: 0 });
}

#[test]
fn everyone_answers_closes_on_time_up() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(1_000, Command::Next).unwrap();
    show.press(2_000, 1, Button::B);

    // Time is up at 11 000; the question stays open for presses still on their way.
    assert_eq!(show.game.due_command(11_000 + PRESS_GRACE_MS - 1), None);
    assert_eq!(show.game.due_command(11_000 + PRESS_GRACE_MS), Some(Command::CloseQuestion));
    let effects = show.run(11_000 + PRESS_GRACE_MS, Command::CloseQuestion).unwrap();
    assert!(effects.contains(&Effect::Sound { cue: Cue::TimeUp }));
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!((show.score(1), show.score(2)), (95, 0));
}

#[test]
fn a_press_counts_by_when_it_was_received() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    for buzzer in [1, 2, 3] {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(1_000, Command::Next).unwrap();
    // Applied after time up (11 000) but received before it: counts.
    show.press(10_900, 1, Button::B);
    // Received after time up: too late, though the question is still open.
    show.press(11_100, 2, Button::B);
    show.run(11_000 + PRESS_GRACE_MS, Command::CloseQuestion).unwrap();
    assert_eq!(show.game.responses().keys().copied().collect::<Vec<_>>(), vec![1]);
    assert_eq!((show.score(1), show.score(2)), (50, 0));
}

#[test]
fn a_late_buzz_does_not_stop_a_fastest_finger_clock() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(10_001, 1, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }));
}

#[test]
fn a_buzz_received_while_another_team_answers_does_not_count() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    show.run(2_000, Command::JudgeWrong).unwrap();
    // Received at 1 500, while buzzer 1 held the clock, applied only now.
    show.press(1_500, 2, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }));
    show.press(2_100, 2, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 2, .. }));
}

#[test]
fn paused_clock_does_not_run_out() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(4_000, Command::PauseClock).unwrap();
    assert_eq!(show.game.due_command(60_000), None);
    show.run(60_000, Command::ResumeClock).unwrap();
    assert_eq!(show.game.clock().unwrap().remaining_ms(61_000), Some(5_000));
}

#[test]
fn fastest_finger_wrong_locks_out_and_resumes_then_right_ends() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(1_000, Command::Next).unwrap();

    let effects = show.press(3_000, 2, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 2, .. }));
    assert!(effects.contains(&Effect::Lights {
        lights: vec![(1, Light::Locked), (2, Light::First), (3, Light::Locked)]
    }));
    // Others cannot buzz while a player is answering.
    show.press(3_100, 3, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 2, .. }));

    show.run(20_000, Command::JudgeWrong).unwrap();
    assert_eq!(show.score(2), -20);
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }));
    // The clock was paused while judging: 2 s used, 8 s left.
    assert_eq!(show.game.clock().unwrap().remaining_ms(20_000), Some(8_000));

    show.press(20_500, 2, Button::Big); // locked out
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }));
    show.press(21_000, 3, Button::Big);
    show.run(25_000, Command::JudgeRight).unwrap();
    // Buzzed after 3 s of clock: 100 - 50 * 3/10 = 85.
    assert_eq!(show.score(3), 85);
    assert_eq!(step(&show.game), QuestionStep::Revealed { winner: Some(3) });
}

#[test]
fn a_fastest_finger_reveal_names_the_winner_whatever_the_points() {
    let mut slide = question("q1", AnswerMode::FastestFinger);
    if let SlideContent::Question(q) = &mut slide.content {
        q.points = 0;
        q.points_end = None;
    }
    let mut show = Show::new(vec![slide]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    show.run(2_000, Command::JudgeRight).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Revealed { winner: Some(1) });
    assert_eq!(show.score(1), 0);
}

#[test]
fn fastest_finger_closes_when_everyone_is_locked_out() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    show.run(2_000, Command::JudgeWrong).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed);
}

#[test]
fn judging_needs_a_buzz_and_next_waits_for_the_verdict() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    assert_eq!(show.run(1, Command::JudgeRight), Err(GameError::NotNow));
    show.press(1_000, 1, Button::Big);
    assert_eq!(show.run(1_001, Command::Next), Err(GameError::NotNow));
}

/// Asserts that `Game::controls` says yes exactly to the step commands
/// `apply` takes, so the Director never offers one the show refuses.
fn assert_controls_match_apply(game: &Game, at_ms: u64) {
    let controls = game.controls();
    let offered = [
        (controls.next, Command::Next),
        (controls.judge, Command::JudgeRight),
        (controls.judge, Command::JudgeWrong),
        (controls.pause_clock, Command::PauseClock),
        (controls.resume_clock, Command::ResumeClock),
        (controls.close_question, Command::CloseQuestion),
    ];
    for (allowed, command) in offered {
        let taken = game.clone().apply(&Stamped { at_ms, command: command.clone() }).is_ok();
        assert_eq!(allowed, taken, "{command:?} in {:?}", game.phase());
    }
}

#[test]
fn controls_offer_exactly_the_commands_each_step_takes() {
    let mut show = Show::new(vec![
        Slide::new("intro", SlideContent::Billboard { title: "Round 1".into(), text: String::new() }),
        question("q1", AnswerMode::FastestFinger),
        Slide::new("why", SlideContent::Explanation { text: "Because.".into() }),
        Slide::new("end", SlideContent::EndOfRound { title: "End".into() }),
    ]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    let buzz = |buzzer| Command::Press { buzzer, button: Button::Big };
    let script = [
        Command::Next,        // intro
        Command::Next,        // q1 ready
        Command::Next,        // running
        Command::PauseClock,  // paused
        Command::ResumeClock, // running
        buzz(1),              // buzzed
        Command::JudgeWrong,  // running again, 1 locked out
        buzz(2),              // buzzed
        Command::JudgeRight,  // revealed
        Command::Next,        // explanation
        Command::Next,        // end of round
        Command::Next,        // final scores
    ];
    let mut at_ms = 0;
    assert_controls_match_apply(&show.game, at_ms);
    for command in script {
        at_ms += 1_000;
        show.run(at_ms, command).unwrap();
        assert_controls_match_apply(&show.game, at_ms);
    }
    while show.game.controls().next {
        show.run(at_ms, Command::Next).unwrap();
        assert_controls_match_apply(&show.game, at_ms);
    }
    assert!(matches!(show.game.phase(), Phase::Final { .. }));

    // A question closed by its clock, then revealed.
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    for (at_ms, command) in [(0, Command::Next), (0, Command::Next), (11_000, Command::CloseQuestion), (12_000, Command::Next)] {
        show.run(at_ms, command).unwrap();
        assert_controls_match_apply(&show.game, at_ms);
    }
}

#[test]
fn scores_stop_at_the_limits_instead_of_wrapping() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.run(0, Command::AddPlayer { buzzer: 1, name: String::new() }).unwrap();
    show.run(0, Command::AdjustScore { buzzer: 1, delta: i32::MAX }).unwrap();
    show.run(0, Command::AdjustScore { buzzer: 1, delta: 1 }).unwrap();
    assert_eq!(show.score(1), i32::MAX);
    show.run(0, Command::AdjustScore { buzzer: 1, delta: i32::MIN }).unwrap();
    show.run(0, Command::AdjustScore { buzzer: 1, delta: i32::MIN }).unwrap();
    assert_eq!(show.score(1), i32::MIN);
}

#[test]
fn ranking_breaks_ties_on_total_answer_time() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.run(0, Command::AddPlayer { buzzer: 1, name: "Slow".into() }).unwrap();
    show.run(0, Command::AddPlayer { buzzer: 2, name: "Fast".into() }).unwrap();
    show.run(0, Command::AdjustScore { buzzer: 1, delta: 50 }).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(9_000, 1, Button::A); // wrong, slow
    show.press(1_000, 2, Button::B); // right after 1 s: 95 points
    show.run(1_000, Command::AdjustScore { buzzer: 2, delta: -25 }).unwrap();
    // Slow: 50 - 20 = 30; Fast: 95 - 25 = 70.
    let names: Vec<&str> = show.game.ranking().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["Fast", "Slow"]);
}

#[test]
fn replaying_the_journal_rebuilds_the_same_game() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone), question("q2", AnswerMode::FastestFinger)]);
    for b in 1..=4 {
        show.press(0, b, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(100, Command::Next).unwrap();
    show.press(1_000, 1, Button::B);
    show.press(2_000, 2, Button::A);
    let _ = show.run(3_000, Command::JudgeRight); // rejected, not journaled
    show.run(11_000, Command::CloseQuestion).unwrap();
    show.run(12_000, Command::Next).unwrap();
    show.run(13_000, Command::Next).unwrap();
    show.run(14_000, Command::Next).unwrap();
    show.press(15_000, 4, Button::Big);

    let replayed = Game::replay(show.game.quiz().clone(), &show.journal);
    assert_eq!(replayed, show.game);
}

#[test]
fn recovery_restarts_a_question_that_was_running() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    assert_eq!(show.game.recovery_command(), None);
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::B);

    let mut crashed = Game::replay(show.game.quiz().clone(), &show.journal);
    let command = crashed.recovery_command().expect("a running question needs recovery");
    crashed.apply(&Stamped { at_ms: 99_000, command }).unwrap();
    assert_eq!(step(&crashed), QuestionStep::Ready);
    assert!(crashed.responses().is_empty());
}

#[test]
fn final_scores_reveal_step_by_step() {
    let mut show = Show::new(vec![]);
    show.run(0, Command::Next).unwrap();
    for expected in 1..=4 {
        show.run(0, Command::Next).unwrap();
        assert_eq!(show.game.phase(), &Phase::Final { step: expected });
    }
    assert_eq!(show.run(0, Command::Next), Err(GameError::NotNow));
}

#[test]
fn an_explanation_follows_the_revealed_question_and_scores_nothing() {
    let explanation = Slide::new("e1", SlideContent::Explanation { text: "Because.".into() });
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone), explanation]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap(); // q1 ready
    show.run(10, Command::Next).unwrap(); // clock runs
    show.press(20, 1, Button::B); // right; the only player, so the question closes
    show.run(30, Command::Next).unwrap(); // reveal
    show.run(40, Command::Next).unwrap(); // explanation
    assert_eq!(show.game.phase(), &Phase::Explanation);
    let score = show.score(1);
    assert!(show.press(50, 1, Button::A).is_empty());
    assert_eq!(show.score(1), score);
    show.run(60, Command::Next).unwrap();
    assert_eq!(show.game.phase(), &Phase::Final { step: 0 });
}

#[test]
fn go_to_final_skips_the_remaining_slides() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone), question("q2", AnswerMode::Everyone)]);
    show.run(0, Command::Next).unwrap();
    show.run(10, Command::Next).unwrap();
    show.run(20, Command::GoToFinal).unwrap();
    assert_eq!(show.game.phase(), &Phase::Final { step: 0 });
    assert!(show.game.current_slide().is_none());
}

#[test]
fn clock_view_carries_the_falling_points() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.run(1_000, Command::Next).unwrap(); // to q1, Ready
    assert_eq!(show.game.clock_view(1_000), None);
    show.run(2_000, Command::Next).unwrap(); // clock starts at 2.0 s, 10 s limit

    let clock = show.game.clock_view(7_000).unwrap();
    assert_eq!(clock.elapsed_ms, 5_000);
    assert_eq!(clock.remaining_ms, Some(5_000));
    assert!(clock.running);
    // 100 falling to 50 over 10 s: halfway is 75.
    assert_eq!(clock.points, 75);
    assert_eq!(show.game.view(7_000).clock, Some(clock));
}

#[test]
fn view_marks_each_answer_right_or_wrong() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    show.run(1_000, Command::Next).unwrap();
    show.run(2_000, Command::Next).unwrap();
    show.press(3_000, 1, Button::B); // correct is index 1
    show.press(3_500, 2, Button::D);

    let view = show.game.view(4_000);
    let correct = |buzzer: u16| view.players.iter().find(|p| p.buzzer == buzzer).unwrap().answer_correct;
    assert_eq!(correct(1), Some(true));
    assert_eq!(correct(2), Some(false));
    assert_eq!(correct(3), None);
}

#[test]
fn view_counts_results_of_enabled_players_only() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    show.run(1_000, Command::Next).unwrap();
    show.run(2_000, Command::Next).unwrap();
    show.press(3_000, 1, Button::B); // right
    show.press(3_500, 2, Button::D); // wrong, then switched off
    show.run(4_000, Command::SetPlayerEnabled { buzzer: 2, enabled: false }).unwrap();
    assert_eq!(show.game.view(4_000).results, None);
    show.run(5_000, Command::CloseQuestion).unwrap();
    let view = show.game.view(5_000);
    assert_eq!(view.results, Some(AnswerResults { right: 1, wrong: 0, none: 1 }));
    assert_eq!(view.answered_count, 1);
}
