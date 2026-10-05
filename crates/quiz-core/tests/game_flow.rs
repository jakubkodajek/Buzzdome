use quiz_core::{
    AnswerMode, Button, Command, Cue, Effect, Game, GameError, Light, Phase, Question, QuestionStep, Quiz, Slide,
    SlideContent, Stamped,
};

fn question(id: &str, mode: AnswerMode) -> Slide {
    Slide {
        id: id.into(),
        notes: String::new(),
        content: SlideContent::Question(Question {
            text: format!("{id}?"),
            mode,
            answers: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            correct: Some(1),
            time_limit_s: 10,
            points: 100,
            points_end: Some(50),
            penalty: 20,
            picture: None,
        }),
    }
}

fn quiz(slides: Vec<Slide>) -> Quiz {
    Quiz { schema_version: 1, title: "Test".into(), slides }
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
    assert_eq!(step(&show.game), QuestionStep::Revealed);
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

    assert_eq!(show.game.due_command(10_999), None);
    assert_eq!(show.game.due_command(11_000), Some(Command::CloseQuestion));
    let effects = show.run(11_000, Command::CloseQuestion).unwrap();
    assert!(effects.contains(&Effect::Sound { cue: Cue::TimeUp }));
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!((show.score(1), show.score(2)), (95, 0));
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
    assert_eq!(step(&show.game), QuestionStep::Revealed);
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

#[test]
fn ranking_breaks_ties_on_total_answer_time() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.run(0, Command::AddPlayer { buzzer: 1, name: "Slow".into() }).unwrap();
    show.run(0, Command::AddPlayer { buzzer: 2, name: "Fast".into() }).unwrap();
    show.run(0, Command::AdjustScore { buzzer: 1, delta: 50 }).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(10_000, 1, Button::A); // wrong, slow
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
