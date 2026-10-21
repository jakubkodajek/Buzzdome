use quiz_core::{
    AnswerMode, BuzzerMode, Button, Command, CorrectAnswer, Cue, Effect, Game, GameError, Light, Phase, Question, QuestionStep, Quiz, Slide,
    SlideContent, Stamped, VideoClip, team_names, FULL_SHARE, MAX_NAME_CHARS, MAX_TEAMS, PRESS_GRACE_MS, RULES_VERSION,
};
use quiz_core::game::FINAL_LAST_STEP;
use quiz_core::view::{AnswerResults, AutoStepView};
use quiz_core::{AllAnswered, AutoStep, QuizSettings};

/// A question; a fastest-finger one is answered aloud, so the big button
/// buzzes in (`a_fastest_finger_question_with_answers_a_to_d_buzzes_with_them`).
fn question(id: &str, mode: AnswerMode) -> Slide {
    let aloud = mode == AnswerMode::FastestFinger;
    Slide::new(
        id,
        SlideContent::Question(Question {
            text: format!("{id}?"),
            mode,
            answers: if aloud { vec![] } else { vec!["A".into(), "B".into(), "C".into(), "D".into()] },
            correct: if aloud { vec![] } else { vec![CorrectAnswer::full(1)] },
            time_limit_s: 10,
            points: 100,
            points_end: Some(50),
            penalty: 20,
        }),
    )
}

fn quiz(slides: Vec<Slide>) -> Quiz {
    Quiz::new("Test", slides)
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

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn teams(game: &Game) -> Vec<(String, Option<u16>)> {
    game.view(0).teams.into_iter().map(|team| (team.name, team.buzzer)).collect()
}

#[test]
fn team_names_are_kept_once_each_without_blanks_and_cut_to_length() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    let long = "x".repeat(MAX_NAME_CHARS + 5);
    show.run(0, Command::SetTeams { names: names(&["  Hawks ", "", "Owls", "hawks", "   ", &long]) }).unwrap();
    assert_eq!(show.game.teams(), names(&["Hawks", "Owls", &"x".repeat(MAX_NAME_CHARS)]).as_slice());

    let too_many: Vec<String> = (0..=MAX_TEAMS).map(|i| format!("Team {i}")).collect();
    assert_eq!(show.run(0, Command::SetTeams { names: too_many }), Err(GameError::TooManyTeams));
    assert_eq!(show.game.teams().len(), 3, "a refused list leaves the old one");
}

#[test]
fn a_list_of_names_counts_teams_not_lines_and_stops_reading_at_the_limit() {
    // Repeats and blanks beyond the limit are not teams.
    let mut lines: Vec<String> = (0..MAX_TEAMS).map(|i| format!("Team {i}")).collect();
    lines.extend(["team 0".to_string(), String::new(), "  ".to_string()]);
    assert_eq!(team_names(lines.iter().map(String::as_str)).map(|teams| teams.len()), Ok(MAX_TEAMS));
    // A wrong file of many lines is refused at the first name too many.
    let lines: Vec<String> = (0..MAX_TEAMS * 100).map(|i| format!("Team {i}")).collect();
    let mut counted = 0;
    let names = lines.iter().map(String::as_str).inspect(|_| counted += 1);
    assert_eq!(team_names(names), Err(GameError::TooManyTeams));
    assert_eq!(counted, MAX_TEAMS + 1);
}

#[test]
fn pairing_gives_a_buzzer_the_team_name_and_moves_it_from_another() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.run(0, Command::SetTeams { names: names(&["Hawks", "Owls"]) }).unwrap();
    show.press(0, 3, Button::Big);
    show.press(0, 5, Button::Big);
    assert_eq!(teams(&show.game), vec![("Hawks".into(), None), ("Owls".into(), None)]);

    show.run(0, Command::PairTeam { buzzer: 3, name: "Hawks".into() }).unwrap();
    assert_eq!(teams(&show.game), vec![("Hawks".into(), Some(3)), ("Owls".into(), None)]);

    // Dragging buzzer 5 onto Hawks takes the name from buzzer 3.
    show.run(0, Command::PairTeam { buzzer: 5, name: "hawks ".into() }).unwrap();
    assert_eq!(teams(&show.game), vec![("Hawks".into(), Some(5)), ("Owls".into(), None)]);
    let name_of = |game: &Game, buzzer| game.players().find(|p| p.buzzer == buzzer).map(|p| p.name.clone());
    assert_eq!(name_of(&show.game, 3).as_deref(), Some(""));

    // Moving buzzer 5 to Owls frees Hawks; renaming a player unpairs it too.
    show.run(0, Command::PairTeam { buzzer: 5, name: "Owls".into() }).unwrap();
    assert_eq!(teams(&show.game), vec![("Hawks".into(), None), ("Owls".into(), Some(5))]);
    show.run(0, Command::RenamePlayer { buzzer: 5, name: String::new() }).unwrap();
    assert_eq!(teams(&show.game), vec![("Hawks".into(), None), ("Owls".into(), None)]);

    assert_eq!(show.run(0, Command::PairTeam { buzzer: 9, name: "Owls".into() }), Err(GameError::UnknownBuzzer(9)));
    // Pairing also works later in the show, for a team that came late.
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::PairTeam { buzzer: 3, name: "Owls".into() }).unwrap();
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION).view(0).teams, show.game.view(0).teams);
}

#[test]
fn renaming_a_player_to_a_name_another_has_takes_it_from_them() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.run(0, Command::SetTeams { names: names(&["Hawks", "Owls"]) }).unwrap();
    show.press(0, 3, Button::Big);
    show.press(0, 5, Button::Big);
    show.run(0, Command::PairTeam { buzzer: 3, name: "Hawks".into() }).unwrap();

    // Typing a paired team's name in the table pairs the team with this buzzer, as dragging does.
    show.run(0, Command::RenamePlayer { buzzer: 5, name: " HAWKS".into() }).unwrap();
    assert_eq!(teams(&show.game), vec![("Hawks".into(), Some(5)), ("Owls".into(), None)]);
    let names_now: Vec<String> = show.game.players().map(|p| p.name.clone()).collect();
    assert_eq!(names_now.iter().filter(|n| n.eq_ignore_ascii_case("hawks")).count(), 1, "{names_now:?}");
    // The same for a name that is no team.
    show.run(0, Command::RenamePlayer { buzzer: 3, name: "Eagles".into() }).unwrap();
    show.run(0, Command::RenamePlayer { buzzer: 5, name: "eagles".into() }).unwrap();
    assert_eq!(show.game.players().filter(|p| p.name.eq_ignore_ascii_case("eagles")).count(), 1);
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    assert!(replayed.players().map(|p| &p.name).eq(show.game.players().map(|p| &p.name)));
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
fn several_correct_answers_each_score_their_share() {
    let mut slide = question("q1", AnswerMode::Everyone);
    if let SlideContent::Question(q) = &mut slide.content {
        q.correct = vec![CorrectAnswer::full(1), CorrectAnswer { answer: 3, share: 50 }];
        q.points_end = None;
    }
    let mut show = Show::new(vec![slide]);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    show.run(1_000, Command::Next).unwrap();
    show.run(2_000, Command::Next).unwrap();
    show.press(3_000, 1, Button::B); // fully right: 100
    show.press(3_000, 2, Button::D); // half right: 50
    show.press(3_000, 3, Button::A); // wrong: -20
    assert_eq!((show.score(1), show.score(2), show.score(3)), (100, 50, -20));
    let view = show.game.view(3_000);
    assert_eq!(view.answer_shares, [None, Some(FULL_SHARE), None, Some(50)]);
    assert_eq!(view.results, Some(AnswerResults { right: 2, wrong: 1, none: 0 }));
    let effects = show.run(4_000, Command::Next).unwrap();
    assert!(effects.contains(&Effect::Lights {
        lights: vec![(1, Light::Correct), (2, Light::Correct), (3, Light::Wrong)]
    }));
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
fn the_first_buzz_applied_wins_though_a_later_one_was_received_earlier() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    // A keyboard press received at 1 000 that waited while a buzzer press received at 1 010 was applied.
    show.press(1_010, 1, Button::Big);
    show.press(1_000, 2, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 1, .. }));
}

#[test]
fn a_press_made_while_the_clock_was_paused_counts_however_late_it_is_applied() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone), question("q2", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(4_000, Command::PauseClock).unwrap();
    show.run(6_000, Command::ResumeClock).unwrap();
    // Received at 5 000 while paused, applied after the resume: counts at the paused time.
    show.press(5_000, 1, Button::B);
    assert_eq!(show.game.responses().get(&1).map(|r| r.elapsed_ms), Some(4_000));

    show.run(7_000, Command::Next).unwrap();
    show.run(7_000, Command::Next).unwrap();
    show.run(8_000, Command::Next).unwrap();
    show.run(9_000, Command::PauseClock).unwrap();
    show.run(12_000, Command::ResumeClock).unwrap();
    show.press(10_000, 1, Button::Big);
    match step(&show.game) {
        QuestionStep::Buzzed { buzzer: 1, clock, .. } => assert_eq!(clock.elapsed_ms(20_000), 1_000),
        other => panic!("the buzz did not count: {other:?}"),
    }
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
fn the_show_holds_while_a_player_answers_the_clock_is_paused_or_the_question_is_over() {
    let intro = Slide::new("next", SlideContent::Billboard { title: "Next".into(), text: String::new() });
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger), intro]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    let mut holds = vec![show.game.on_hold()];
    let mut run = |show: &mut Show, at_ms: u64, command: Command| {
        show.run(at_ms, command).unwrap();
        holds.push(show.game.on_hold());
    };
    run(&mut show, 0, Command::Next); // ready
    run(&mut show, 0, Command::Next); // running
    run(&mut show, 1_000, Command::PauseClock);
    run(&mut show, 2_000, Command::ResumeClock);
    run(&mut show, 3_000, Command::Press { buzzer: 1, button: Button::Big });
    run(&mut show, 4_000, Command::JudgeWrong); // running again
    run(&mut show, 5_000, Command::CloseQuestion);
    run(&mut show, 6_000, Command::Next); // revealed
    run(&mut show, 7_000, Command::Next); // the billboard
    assert_eq!(holds, [false, false, false, true, false, true, false, true, true, false]);
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
fn the_buzzer_mode_follows_the_steps_of_a_show() {
    let mut show = Show::new(vec![
        question("q1", AnswerMode::FastestFinger),
        question("q2", AnswerMode::Everyone),
        question("q3", AnswerMode::FastestFinger),
    ]);
    let mode = |show: &Show| show.game.buzzer_mode();
    assert_eq!(mode(&show), BuzzerMode::Register);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Off);
    show.run(1_000, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::FastestFinger);
    show.press(2_000, 1, Button::Big);
    assert_eq!(mode(&show), BuzzerMode::Buzzed);
    show.run(3_000, Command::JudgeWrong).unwrap();
    assert_eq!(mode(&show), BuzzerMode::FastestFinger);
    // A second wrong answer goes through a buzz again, so the round goes
    // on once more.
    show.press(4_000, 2, Button::Big);
    assert_eq!(mode(&show), BuzzerMode::Buzzed);
    show.run(5_000, Command::JudgeWrong).unwrap();
    assert_eq!(mode(&show), BuzzerMode::FastestFinger);
    show.press(6_000, 3, Button::Big);
    show.run(7_000, Command::JudgeRight).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Results);

    show.run(8_000, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Off);
    show.run(9_000, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Answers);
    show.press(10_000, 1, Button::B);
    assert_eq!(mode(&show), BuzzerMode::Answers);
    // A restart begins the question again from its start, with a new round.
    show.run(11_000, Command::RestartQuestion).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Off);
    show.run(12_000, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Answers);
    show.run(13_000, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Off);
    show.run(14_000, Command::Next).unwrap();
    assert_eq!(mode(&show), BuzzerMode::Results);

    // A fastest-finger question nobody won shows no result.
    show.run(15_000, Command::Next).unwrap();
    show.run(16_000, Command::Next).unwrap();
    show.run(17_000, Command::Next).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed);
    show.run(18_000, Command::Next).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Revealed { winner: None });
    assert_eq!(mode(&show), BuzzerMode::Off);
}

#[test]
fn the_lights_of_a_show_are_every_players_with_switched_off_ones_locked() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    for b in 1..=3 {
        show.press(0, b, Button::Big);
    }
    assert_eq!(show.game.lights(), [(1, Light::Ready), (2, Light::Ready), (3, Light::Ready)]);
    show.run(0, Command::SetPlayerEnabled { buzzer: 3, enabled: false }).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(1_000, Command::Next).unwrap();
    show.press(2_000, 1, Button::Big);
    assert_eq!(show.game.lights(), [(1, Light::First), (2, Light::Locked), (3, Light::Locked)]);
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
        (controls.play_video, Command::PlayVideo),
        (controls.go_to, Command::GoTo { slide: 0 }),
        (controls.go_to, Command::GoToFinal),
        (controls.restart_question, Command::RestartQuestion),
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
        with_video(question("q1", AnswerMode::FastestFinger)),
        Slide::new("why", SlideContent::Explanation { text: "Because.".into() }),
        Slide::new("end", SlideContent::EndOfRound { title: "End".into() }),
        final_slide("final"),
        billboard("thanks"),
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
        Command::Next,        // final slide
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
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("thanks"));

    // A question closed by its clock, then revealed.
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    for (at_ms, command) in [(0, Command::Next), (0, Command::Next), (11_000, Command::CloseQuestion), (12_000, Command::Next)] {
        show.run(at_ms, command).unwrap();
        assert_controls_match_apply(&show.game, at_ms);
    }
}

/// Checks that `ignores` says of every press, of known buzzers and a new one,
/// exactly whether applying it leaves the show as it is.
fn assert_ignores_match_apply(game: &Game, at_ms: u64) {
    for buzzer in 1..=4 {
        for button in [Button::A, Button::B, Button::C, Button::D, Button::Big] {
            let stamped = Stamped { at_ms, command: Command::Press { buzzer, button } };
            let mut applied = game.clone();
            applied.apply(&stamped).unwrap();
            assert_eq!(game.ignores(&stamped), applied == *game, "{stamped:?} in {:?}", game.phase());
        }
    }
}

#[test]
fn a_press_is_ignored_exactly_when_it_changes_nothing() {
    // Fastest finger: before the clock, running, paused, buzzed, locked out, revealed.
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger), question("q2", AnswerMode::Everyone)]);
    for buzzer in 1..=3 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::SetPlayerEnabled { buzzer: 3, enabled: false }).unwrap();
    let buzz = |buzzer| Command::Press { buzzer, button: Button::Big };
    let script = [
        (0, Command::Next),            // q1 ready
        (1_000, Command::Next),        // running
        (2_000, Command::PauseClock),  // paused
        (3_000, Command::ResumeClock), // running
        (4_000, buzz(1)),              // buzzed
        (5_000, Command::JudgeWrong),  // running again, 1 locked out
        (6_000, buzz(2)),              // buzzed
        (7_000, Command::JudgeRight),  // revealed
        (8_000, Command::Next),        // q2 ready
        (9_000, Command::Next),        // running
        (10_000, Command::Press { buzzer: 1, button: Button::B }), // 1 answered
    ];
    assert_ignores_match_apply(&show.game, 0);
    for (at_ms, command) in script {
        show.run(at_ms, command).unwrap();
        assert_ignores_match_apply(&show.game, at_ms);
    }
    // Before the question started, after time up and once it is closed.
    assert_ignores_match_apply(&show.game, 8_500);
    assert_ignores_match_apply(&show.game, 19_000 + PRESS_GRACE_MS);
    show.run(20_000, Command::CloseQuestion).unwrap();
    assert_ignores_match_apply(&show.game, 20_000);
    assert!(show.game.ignores(&Stamped { at_ms: 20_000, command: buzz(1) }));
    assert!(!show.game.ignores(&Stamped { at_ms: 20_000, command: Command::Next }), "only presses are ignored");
}

fn with_video(mut slide: Slide) -> Slide {
    slide.video = Some(VideoClip::new("media/clip.mp4"));
    slide
}

#[test]
fn the_quizmaster_starts_a_slide_video_as_often_as_needed_until_the_slide_changes() {
    let end = with_video(Slide::new("end", SlideContent::EndOfRound { title: "End".into() }));
    let mut show = Show::new(vec![with_video(question("q1", AnswerMode::Everyone)), billboard("plain"), end]);
    show.press(0, 1, Button::Big);
    assert_eq!(show.run(0, Command::PlayVideo), Err(GameError::NotNow), "nothing is shown in the lobby");
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::PlayVideo).unwrap();
    show.run(0, Command::Next).unwrap(); // running
    show.run(0, Command::PlayVideo).unwrap();
    assert_eq!(show.game.view(0).video_plays, 2);
    show.press(1_000, 1, Button::B); // closes the question: everyone answered
    show.run(0, Command::PlayVideo).unwrap();
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION), show.game);

    show.run(0, Command::Next).unwrap(); // revealed
    show.run(0, Command::Next).unwrap(); // a slide without a video
    assert_eq!(show.game.view(0).video_plays, 0);
    assert_eq!(show.run(0, Command::PlayVideo), Err(GameError::NotNow));
    // A ranking has no room for a video, whatever the file says.
    show.run(0, Command::Next).unwrap();
    assert_eq!(show.run(0, Command::PlayVideo), Err(GameError::NotNow));
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

/// A question that scores the same however late the answer.
fn flat(mut slide: Slide) -> Slide {
    if let SlideContent::Question(q) = &mut slide.content {
        q.points_end = None;
    }
    slide
}

#[test]
fn ranking_breaks_ties_on_the_time_of_right_answers_and_equal_players_share_a_place() {
    let mut show = Show::new(vec![flat(question("q1", AnswerMode::Everyone))]);
    for buzzer in 1..=5 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(3_000, 1, Button::B); // right after 3 s: 100
    show.press(1_000, 2, Button::B); // right after 1 s: 100
    show.press(500, 3, Button::A); // wrong: -20, and its time does not count
    show.run(4_000, Command::CloseQuestion).unwrap();
    let answering: Vec<u64> = show.game.players().map(|p| p.total_answer_ms).collect();
    assert_eq!(answering, [3_000, 1_000, 0, 0, 0]);

    let ranking: Vec<(usize, u16)> = show.game.ranking().into_iter().map(|(place, p)| (place, p.buzzer)).collect();
    // 4 and 5 neither scored nor answered right: they share third place, and nobody is fourth.
    assert_eq!(ranking, [(1, 2), (2, 1), (3, 4), (3, 5), (5, 3)]);
    let ranks: Vec<(u16, Option<usize>)> = show.game.view(4_000).players.iter().map(|p| (p.buzzer, p.rank)).collect();
    assert_eq!(ranks, [(2, Some(1)), (1, Some(2)), (4, Some(3)), (5, Some(3)), (3, Some(5))]);
}

#[test]
fn replaying_the_journal_rebuilds_the_same_game() {
    let mut show = Show::new(vec![with_video(question("q1", AnswerMode::Everyone)), question("q2", AnswerMode::FastestFinger)]);
    for b in 1..=4 {
        show.press(0, b, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(100, Command::Next).unwrap();
    show.run(500, Command::PlayVideo).unwrap();
    show.press(1_000, 1, Button::B);
    show.press(2_000, 2, Button::A);
    let _ = show.run(3_000, Command::JudgeRight); // rejected, not journaled
    show.run(11_000, Command::CloseQuestion).unwrap();
    show.run(12_000, Command::Next).unwrap();
    show.run(13_000, Command::Next).unwrap();
    show.run(14_000, Command::Next).unwrap();
    show.press(15_000, 4, Button::Big);

    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
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

    let mut crashed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    let command = crashed.recovery_command().expect("a running question needs recovery");
    assert_eq!(command, Command::RestartQuestion);
    crashed.apply(&Stamped { at_ms: 99_000, command }).unwrap();
    assert_eq!(step(&crashed), QuestionStep::Ready);
    assert!(crashed.responses().is_empty());
    assert_eq!(crashed.recovery_command(), None);
}

#[test]
fn a_journal_begun_under_rules_version_1_replays_its_restart_by_go_to() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::B);
    // Version 1 restarted the question after a crash with a jump to it.
    let mut journal = show.journal.clone();
    journal.push(Stamped { at_ms: 60_000, command: Command::GoTo { slide: 0 } });
    journal.push(Stamped { at_ms: 61_000, command: Command::Next });

    let replayed = Game::replay(show.game.quiz().clone(), &journal, 1);
    assert!(matches!(step(&replayed), QuestionStep::Running { .. }), "{:?}", replayed.phase());
    assert!(replayed.responses().is_empty(), "the question started again");
    // Today's rules refuse that jump, so the Next would close the question instead.
    let today = Game::replay(show.game.quiz().clone(), &journal, RULES_VERSION);
    assert_eq!(step(&today), QuestionStep::Closed);
    // Resumed, the show goes on under version 1, so the jump and the Next do as they did then.
    let mut resumed = Game::replay(show.game.quiz().clone(), &show.journal, 1);
    assert!(resumed.controls().go_to, "version 1 jumps out of a question in play");
    for stamped in &journal[show.journal.len()..] {
        resumed.apply(stamped).unwrap();
    }
    assert_eq!(resumed, replayed);
}

#[test]
fn a_wrong_answer_before_a_crash_is_charged_once_after_it() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    show.run(1_500, Command::JudgeWrong).unwrap();
    assert_eq!(show.score(1), -20);

    // The app stops; the resumed show restarts the question, penalty and all.
    let mut resumed = Show { game: Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION), journal: show.journal.clone() };
    let restart = resumed.game.recovery_command().expect("the question was in play");
    resumed.run(60_000, restart).unwrap();
    assert_eq!(resumed.score(1), 0);
    assert!(resumed.game.view(60_000).players.iter().all(|p| p.delta.is_none()), "no score change shown from before");
    resumed.run(61_000, Command::Next).unwrap();
    resumed.press(62_000, 1, Button::Big);
    resumed.run(62_500, Command::JudgeWrong).unwrap();
    assert_eq!(resumed.score(1), -20);
    assert_eq!(Game::replay(resumed.game.quiz().clone(), &resumed.journal, RULES_VERSION), resumed.game);
}

/// An everyone-answers question with a manual score change, two answers and
/// time run out: closed, its results on the projector.
fn closed_on_time_up() -> Show {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    for buzzer in 1..=3 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::AdjustScore { buzzer: 1, delta: 5 }).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(2_000, 1, Button::B);
    show.press(4_000, 2, Button::A);
    let due = show.game.due_command(10_000 + PRESS_GRACE_MS).unwrap();
    show.run(10_000 + PRESS_GRACE_MS, due).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!((show.score(1), show.score(2)), (5 + 90, -20));
    assert!(show.game.view(10_000 + PRESS_GRACE_MS).results.is_some(), "the results are on screen");
    show
}

#[test]
fn a_closed_question_does_not_restart_as_its_results_are_on_screen() {
    let mut show = closed_on_time_up();
    assert!(!show.game.controls().restart_question);
    assert_eq!(show.run(11_000, Command::RestartQuestion), Err(GameError::NotNow));
    show.run(12_000, Command::Next).unwrap();
    assert_eq!(show.run(13_000, Command::RestartQuestion), Err(GameError::NotNow), "nor once revealed");
}

#[test]
fn a_journal_begun_under_rules_version_3_restarts_a_closed_question_as_then() {
    let show = closed_on_time_up();
    let mut journal = show.journal.clone();
    journal.push(Stamped { at_ms: 11_000, command: Command::RestartQuestion });

    let replayed = Game::replay(show.game.quiz().clone(), &journal, 3);
    assert_eq!(step(&replayed), QuestionStep::Ready);
    let scores: Vec<i32> = replayed.players().map(|p| p.score).collect();
    assert_eq!(scores, [5, 0, 0], "a manual change stays");
    let answering: Vec<u64> = replayed.players().map(|p| p.total_answer_ms).collect();
    assert_eq!(answering, [0, 0, 0], "the answering times go with the answers");
    assert!(replayed.view(11_000).players.iter().all(|p| p.delta.is_none()));

    let today = Game::replay(show.game.quiz().clone(), &journal, RULES_VERSION);
    assert_eq!(step(&today), QuestionStep::Closed, "a restart refused today was never journaled");
}

#[test]
fn a_fastest_finger_question_closed_by_wrong_answers_or_time_up_restarts_only_under_older_rules() {
    for by_time in [false, true] {
        let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
        show.press(0, 1, Button::Big);
        show.press(0, 2, Button::Big);
        show.run(0, Command::Next).unwrap();
        show.run(0, Command::Next).unwrap();
        show.press(1_000, 1, Button::Big);
        show.run(2_000, Command::JudgeWrong).unwrap();
        if by_time {
            let due = show.game.due_command(11_000 + PRESS_GRACE_MS).unwrap();
            show.run(11_000 + PRESS_GRACE_MS, due).unwrap();
        } else {
            show.press(3_000, 2, Button::Big);
            show.run(4_000, Command::JudgeWrong).unwrap();
        }
        assert_eq!(step(&show.game), QuestionStep::Closed, "closed by time: {by_time}");
        assert_eq!(show.run(20_000, Command::RestartQuestion), Err(GameError::NotNow));

        let mut journal = show.journal.clone();
        journal.push(Stamped { at_ms: 20_000, command: Command::RestartQuestion });
        let replayed = Game::replay(show.game.quiz().clone(), &journal, 3);
        assert_eq!(step(&replayed), QuestionStep::Ready, "closed by time: {by_time}");
        assert!(replayed.players().all(|p| p.score == 0), "the penalties are taken back");
    }
}

#[test]
fn a_restart_takes_back_the_penalty_of_a_player_switched_off_since() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    for buzzer in 1..=3 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    show.run(2_000, Command::JudgeWrong).unwrap();
    show.run(3_000, Command::SetPlayerEnabled { buzzer: 1, enabled: false }).unwrap();
    assert_eq!(show.score(1), -20);
    show.run(4_000, Command::RestartQuestion).unwrap();
    assert_eq!(show.score(1), 0);
    assert_eq!(step(&show.game), QuestionStep::Ready);
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION), show.game);
}

#[test]
fn a_question_in_play_cannot_be_jumped_away_from() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger), question("q2", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    assert!(show.game.controls().go_to, "a question not started yet can be left");
    assert_eq!(show.run(0, Command::RestartQuestion), Err(GameError::NotNow));
    show.run(0, Command::Next).unwrap();
    for buzzed in [false, true] {
        if buzzed {
            show.press(1_000, 1, Button::Big);
        }
        assert!(!show.game.controls().go_to);
        assert_eq!(show.run(2_000, Command::GoTo { slide: 1 }), Err(GameError::NotNow));
        assert_eq!(show.run(2_000, Command::GoToFinal), Err(GameError::NotNow));
    }
    show.run(3_000, Command::JudgeRight).unwrap();
    assert!(show.game.controls().go_to);
    show.run(4_000, Command::GoTo { slide: 1 }).unwrap();
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

fn final_slide(id: &str) -> Slide {
    Slide::new(id, SlideContent::Final { title: "Winners".into() })
}

fn billboard(id: &str) -> Slide {
    Slide::new(id, SlideContent::Billboard { title: id.into(), text: String::new() })
}

#[test]
fn a_final_slide_shows_the_ranking_where_it_stands_and_the_show_ends_with_the_last_slide() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone), final_slide("final"), billboard("thanks")]);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::GoTo { slide: 1 }).unwrap();
    assert_eq!(show.game.phase(), &Phase::Final { step: 0 });
    // The final slide is on screen, so its sound and background play.
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("final"));
    assert_eq!(show.game.view(0).next_slide, Some(2));
    for expected in 1..=4 {
        show.run(0, Command::Next).unwrap();
        assert_eq!(show.game.phase(), &Phase::Final { step: expected });
    }
    show.run(0, Command::Next).unwrap();
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("thanks"));
    // No second ranking after the last slide: the quiz has its own.
    assert!(!show.game.controls().next);
    assert_eq!(show.run(0, Command::Next), Err(GameError::NotNow));
    assert_eq!(show.game.view(0).next_slide, None);

    show.run(0, Command::GoToFinal).unwrap();
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("final"));
    assert_eq!(show.game.phase(), &Phase::Final { step: 0 });
}

#[test]
fn a_final_slide_at_the_end_ends_the_show_after_the_full_ranking() {
    let mut show = Show::new(vec![billboard("intro"), final_slide("final")]);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("final"));
    while show.game.controls().next {
        show.run(0, Command::Next).unwrap();
    }
    assert_eq!(show.game.phase(), &Phase::Final { step: FINAL_LAST_STEP });
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("final"));
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
fn view_counts_results_of_the_players_the_question_counted() {
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

#[test]
fn switching_off_the_last_player_to_answer_closes_the_question_and_later_switches_change_nothing() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    for buzzer in 1..=3 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::B); // right
    show.press(2_000, 2, Button::A); // wrong
    let effects = show.run(3_000, Command::SetPlayerEnabled { buzzer: 3, enabled: false }).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed, "nobody is left to answer");
    assert_eq!(effects, [Effect::Lights { lights: vec![(3, Light::Locked)] }, Effect::Lights { lights: vec![(1, Light::Off), (2, Light::Off)] }]);
    assert_eq!((show.score(1), show.score(2), show.score(3)), (95, -20, 0));

    // Who counts was settled when the question closed.
    show.run(4_000, Command::SetPlayerEnabled { buzzer: 1, enabled: false }).unwrap();
    let effects = show.run(4_000, Command::SetPlayerEnabled { buzzer: 3, enabled: true }).unwrap();
    assert_eq!(effects, [Effect::Lights { lights: vec![(3, Light::Off)] }]);
    let view = show.game.view(4_000);
    assert_eq!(view.results, Some(AnswerResults { right: 1, wrong: 1, none: 0 }));
    assert_eq!(view.answer_counts, Some(vec![1, 1, 0, 0]));
    show.run(5_000, Command::SetPlayerEnabled { buzzer: 1, enabled: true }).unwrap();
    let effects = show.run(6_000, Command::Next).unwrap();
    assert!(effects.contains(&Effect::Lights { lights: vec![(1, Light::Correct), (2, Light::Wrong), (3, Light::Off)] }));
    assert_eq!(show.game.counted().iter().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION), show.game);
}

#[test]
fn a_player_switched_on_mid_question_joins_at_the_next_and_one_switched_off_cannot_buzz() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger), question("q2", AnswerMode::FastestFinger)]);
    for buzzer in 1..=3 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::SetPlayerEnabled { buzzer: 3, enabled: false }).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    assert!(show.press(500, 3, Button::Big).is_empty(), "switched off");
    // The others' buzzer timers count from the start, so 3 joins at the next question.
    let effects = show.run(600, Command::SetPlayerEnabled { buzzer: 3, enabled: true }).unwrap();
    assert_eq!(effects, [Effect::Lights { lights: vec![(3, Light::Locked)] }]);
    assert!(show.press(700, 3, Button::Big).is_empty(), "joins at the next question");

    show.press(1_000, 1, Button::Big);
    let effects = show.run(2_000, Command::JudgeWrong).unwrap();
    assert_eq!(show.score(1), -20);
    assert!(effects.contains(&Effect::Lights { lights: vec![(2, Light::Ready), (3, Light::Locked)] }));
    show.run(2_500, Command::SetPlayerEnabled { buzzer: 1, enabled: false }).unwrap();
    show.run(2_500, Command::SetPlayerEnabled { buzzer: 1, enabled: true }).unwrap();
    assert!(show.press(3_000, 1, Button::Big).is_empty(), "still locked out");
    // Only 2 is left; switching it off leaves nobody to buzz.
    let effects = show.run(5_000, Command::SetPlayerEnabled { buzzer: 2, enabled: false }).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!(effects[1], Effect::Lights { lights: vec![(1, Light::Off), (3, Light::Off)] });
    let answering: Vec<u64> = show.game.players().map(|p| p.total_answer_ms).collect();
    assert_eq!(answering, [0, 0, 0], "wrong answers add no answering time");
    assert_eq!(show.game.view(5_000).answer_counts, None, "fastest finger counts no answers");

    show.run(6_000, Command::Next).unwrap();
    show.run(6_000, Command::Next).unwrap();
    show.run(6_000, Command::Next).unwrap();
    show.press(7_000, 3, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 3, .. }), "in from the next question");
}

#[test]
fn enabling_players_who_are_on_leaves_them_in_the_fastest_finger_question() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    for buzzer in 1..=2 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    // The Teams tab sends Enable for every selected row.
    for buzzer in 1..=2 {
        let effects = show.run(500, Command::SetPlayerEnabled { buzzer, enabled: true }).unwrap();
        assert_eq!(effects, [Effect::Lights { lights: vec![(buzzer, Light::Ready)] }]);
    }
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }), "still open");
    assert!(show.game.view(500).players.iter().all(|p| !p.joins_next && !p.locked_out));
    show.press(1_000, 2, Button::Big);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 2, .. }));
}

#[test]
fn a_buzzer_new_to_the_show_mid_fastest_finger_plays_from_the_next_question() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger), question("q2", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    // A press registers a buzzer whenever it comes, an added player too.
    assert_eq!(show.press(500, 2, Button::Big), [Effect::Lights { lights: vec![(2, Light::Locked)] }]);
    assert_eq!(show.run(500, Command::AddPlayer { buzzer: 3, name: "Late".into() }).unwrap(), [Effect::Lights { lights: vec![(3, Light::Locked)] }]);
    assert!(show.press(700, 3, Button::Big).is_empty(), "plays from the next question");
    let view = show.game.view(700);
    let joining: Vec<u16> = view.players.iter().filter(|p| p.joins_next).map(|p| p.buzzer).collect();
    assert_eq!(joining, [2, 3]);
    assert!(view.players.iter().all(|p| !p.locked_out), "not marked as answered wrong");
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }), "1 is left to buzz");

    // In the next question, everyone answers; a buzzer new to it answers at once.
    show.run(1_000, Command::CloseQuestion).unwrap();
    show.run(1_000, Command::Next).unwrap();
    show.run(1_000, Command::Next).unwrap();
    show.run(1_000, Command::Next).unwrap();
    assert!(show.game.view(1_000).players.iter().all(|p| !p.joins_next));
    show.press(1_500, 4, Button::B);
    assert_eq!(show.game.responses().get(&4).map(|r| r.button), Some(Button::B));
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION), show.game);
}

#[test]
fn a_player_switched_on_mid_question_could_buzz_at_once_under_older_rules() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    for buzzer in 1..=2 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::SetPlayerEnabled { buzzer: 2, enabled: false }).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.run(600, Command::SetPlayerEnabled { buzzer: 2, enabled: true }).unwrap();
    show.journal.push(Stamped { at_ms: 1_000, command: Command::Press { buzzer: 2, button: Button::Big } });
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, 4);
    assert!(matches!(step(&replayed), QuestionStep::Buzzed { buzzer: 2, .. }));
}

/// A fastest-finger question with `answers` answers.
fn fastest_finger_with(answers: usize) -> Slide {
    let mut slide = question("ff", AnswerMode::FastestFinger);
    let SlideContent::Question(q) = &mut slide.content else { unreachable!() };
    q.answers = (0..answers).map(|i| format!("answer {i}")).collect();
    q.correct = vec![CorrectAnswer::full(0)];
    slide
}

/// Which of A, B, C, D and the big button buzz in on `slide`, as their
/// answer letters, `*` for the big one.
fn buzz_buttons(slide: Slide, rules_version: u32) -> String {
    let buttons = [(Button::A, 'A'), (Button::B, 'B'), (Button::C, 'C'), (Button::D, 'D'), (Button::Big, '*')];
    buttons
        .into_iter()
        .filter(|&(button, _)| {
            let mut show = Show::new(vec![slide.clone()]);
            show.press(0, 1, Button::Big);
            show.run(0, Command::Next).unwrap();
            show.run(0, Command::Next).unwrap();
            show.journal.push(Stamped { at_ms: 1_000, command: Command::Press { buzzer: 1, button } });
            let game = Game::replay(show.game.quiz().clone(), &show.journal, rules_version);
            matches!(step(&game), QuestionStep::Buzzed { buzzer: 1, button: buzzed, .. } if buzzed == button)
        })
        .map(|(_, letter)| letter)
        .collect()
}

#[test]
fn a_fastest_finger_question_with_answers_a_to_d_buzzes_with_them() {
    assert_eq!(buzz_buttons(fastest_finger_with(4), RULES_VERSION), "ABCD");
    assert_eq!(buzz_buttons(fastest_finger_with(3), RULES_VERSION), "ABC", "no answer D");
    assert_eq!(buzz_buttons(fastest_finger_with(1), RULES_VERSION), "A");
    // Answered aloud, or with E and F read out: the big button.
    assert_eq!(buzz_buttons(fastest_finger_with(0), RULES_VERSION), "*");
    assert_eq!(buzz_buttons(fastest_finger_with(5), RULES_VERSION), "*");
    assert_eq!(buzz_buttons(fastest_finger_with(6), RULES_VERSION), "*");
    // Before rules version 5 any button buzzed.
    assert_eq!(buzz_buttons(fastest_finger_with(4), 4), "ABCD*");
    assert_eq!(buzz_buttons(fastest_finger_with(0), 4), "ABCD*");
}

#[test]
fn a_button_that_does_not_buzz_in_is_ignored_and_the_buzz_keeps_its_answer() {
    let mut show = Show::new(vec![fastest_finger_with(4)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::A);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    let big = Stamped { at_ms: 1_000, command: Command::Press { buzzer: 1, button: Button::Big } };
    assert!(show.game.ignores(&big), "neither journaled nor shown");
    show.press(1_100, 2, Button::C);
    assert!(matches!(step(&show.game), QuestionStep::Buzzed { buzzer: 2, button: Button::C, .. }));
    assert_eq!(show.game.buzzer_mode(), BuzzerMode::Buzzed);
}

#[test]
fn the_lights_of_a_wrong_judgement_merge_into_one_per_buzzer() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    let effects = show.run(2_000, Command::JudgeWrong).unwrap();
    assert_eq!(Effect::lights(&effects), [(1, Light::Wrong), (2, Light::Ready)]);
    // The last wrong answer closes the question: the others go dark.
    show.press(3_000, 2, Button::Big);
    let effects = show.run(4_000, Command::JudgeWrong).unwrap();
    assert_eq!(Effect::lights(&effects), [(1, Light::Off), (2, Light::Wrong)]);
    assert!(Effect::lights(&[Effect::Sound { cue: Cue::Wrong }]).is_empty());
}

#[test]
fn the_last_wrong_buzz_closes_the_question_and_darkens_the_other_lights() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    show.run(2_000, Command::JudgeWrong).unwrap();
    show.press(3_000, 2, Button::Big);
    let effects = show.run(4_000, Command::JudgeWrong).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!(
        effects,
        [
            Effect::Lights { lights: vec![(2, Light::Wrong)] },
            Effect::Sound { cue: Cue::Wrong },
            Effect::Lights { lights: vec![(1, Light::Off)] },
        ]
    );
    // The reveal darkens it too, as nobody won.
    let effects = show.run(5_000, Command::Next).unwrap();
    assert_eq!(effects, [Effect::Lights { lights: vec![(1, Light::Off), (2, Light::Off)] }, Effect::Sound { cue: Cue::Reveal }]);
}

#[test]
fn a_wrong_buzz_shows_wrong_until_the_next_buzz() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    for buzzer in 1..=3 {
        show.press(0, buzzer, Button::Big);
    }
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    let effects = show.run(2_000, Command::JudgeWrong).unwrap();
    assert_eq!(
        effects,
        [
            Effect::Lights { lights: vec![(1, Light::Wrong)] },
            Effect::Sound { cue: Cue::Wrong },
            Effect::Lights { lights: vec![(2, Light::Ready), (3, Light::Ready)] },
        ]
    );
    let effects = show.press(3_000, 2, Button::Big);
    assert!(effects.contains(&Effect::Lights { lights: vec![(1, Light::Locked), (2, Light::First), (3, Light::Locked)] }), "{effects:?}");
}

#[test]
fn a_journal_begun_under_rules_version_2_replays_switching_a_player_off_as_then() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::B);
    let mut journal = show.journal.clone();
    journal.push(Stamped { at_ms: 2_000, command: Command::SetPlayerEnabled { buzzer: 2, enabled: false } });
    // Version 2 left the question open, so the quizmaster closed it with Next.
    journal.push(Stamped { at_ms: 3_000, command: Command::Next });

    let replayed = Game::replay(show.game.quiz().clone(), &journal, 2);
    assert_eq!(step(&replayed), QuestionStep::Closed);
    let today = Game::replay(show.game.quiz().clone(), &journal, RULES_VERSION);
    assert_eq!(step(&today), QuestionStep::Revealed { winner: None }, "today the switch closes it");
    // Resumed, the show goes on under version 2, so the switch and the Next do as they did then.
    let mut resumed = Game::replay(show.game.quiz().clone(), &show.journal, 2);
    for stamped in &journal[show.journal.len()..] {
        resumed.apply(stamped).unwrap();
    }
    assert_eq!(resumed, replayed);
}

#[test]
fn recovery_restarts_a_question_whose_buzz_waited_for_judgement() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger)]);
    show.press(0, 1, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    show.press(1_000, 1, Button::Big);
    let mut crashed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    let command = crashed.recovery_command().expect("a buzz waiting for judgement needs recovery");
    crashed.apply(&Stamped { at_ms: 60_000, command }).unwrap();
    assert_eq!(step(&crashed), QuestionStep::Ready);
    assert_eq!(crashed.players().map(|p| p.score).collect::<Vec<_>>(), [0]);
}

#[test]
fn an_untimed_question_never_runs_out_and_scores_its_full_points() {
    let mut slide = question("q1", AnswerMode::Everyone);
    if let SlideContent::Question(q) = &mut slide.content {
        q.time_limit_s = 0;
    }
    let mut show = Show::new(vec![slide]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(0, Command::Next).unwrap();
    show.run(0, Command::Next).unwrap();
    let late = 3_600_000;
    assert_eq!(show.game.due_command(late), None);
    let clock = show.game.clock_view(late).unwrap();
    assert_eq!((clock.remaining_ms, clock.elapsed_ms, clock.points), (None, late, 100));
    show.press(late, 1, Button::B);
    show.run(late + 1, Command::Next).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!((show.score(1), show.score(2)), (100, 0));
}

#[test]
fn a_jump_past_the_last_slide_and_a_second_player_on_one_buzzer_are_refused() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    assert_eq!(show.run(0, Command::GoTo { slide: 1 }), Err(GameError::NoSuchSlide(1)));
    show.run(0, Command::AddPlayer { buzzer: 1, name: "Ann".into() }).unwrap();
    assert_eq!(show.run(0, Command::AddPlayer { buzzer: 1, name: "Bob".into() }), Err(GameError::DuplicateBuzzer(1)));
    assert_eq!(show.game.players().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Ann"]);
}

#[test]
fn a_name_is_the_same_however_its_accents_are_encoded() {
    let mut show = Show::new(vec![question("q1", AnswerMode::Everyone)]);
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    // As pasted from a macOS file name: "e" and an accent rather than "é".
    let decomposed = "Z\u{30c}lut\u{30c}ouc\u{30c}ky\u{301}";
    show.run(0, Command::SetTeams { names: names(&["Žluťoučký", decomposed, "Kůň"]) }).unwrap();
    assert_eq!(show.game.teams(), ["Žluťoučký", "Kůň"]);
    show.run(0, Command::PairTeam { buzzer: 1, name: decomposed.into() }).unwrap();
    assert_eq!(teams(&show.game), [("Žluťoučký".to_string(), Some(1)), ("Kůň".to_string(), None)]);
    assert_eq!(show.game.players().next().unwrap().name, "Žluťoučký", "kept composed");
    show.run(0, Command::RenamePlayer { buzzer: 2, name: "žluťoučký".into() }).unwrap();
    let players: Vec<&str> = show.game.players().map(|p| p.name.as_str()).collect();
    assert_eq!(players, ["", "žluťoučký"], "the name moved to the other buzzer");
}

/// A show of `slides` whose quiz takes `all_answered` after `delay_s`.
fn show_with(slides: Vec<Slide>, all_answered: AllAnswered, delay_s: u8) -> Show {
    let quiz = Quiz { settings: QuizSettings { all_answered, all_answered_delay_s: delay_s }, ..quiz(slides) };
    Show { game: Game::new(quiz), journal: Vec::new() }
}

/// Two players, and the first slide's question running from 2 s.
fn running(mut show: Show) -> Show {
    show.press(0, 1, Button::Big);
    show.press(0, 2, Button::Big);
    show.run(1_000, Command::Next).unwrap();
    show.run(2_000, Command::Next).unwrap();
    show
}

/// Applies the command due at `now`, as the app's show thread does.
fn run_due(show: &mut Show, now: u64) {
    let due = show.game.due_command(now).expect("a command is due");
    show.run(now, due).unwrap();
}

#[test]
fn by_default_a_question_everyone_answered_waits_for_next() {
    let mut show = running(Show::new(vec![question("q1", AnswerMode::Everyone)]));
    show.press(3_000, 1, Button::B);
    show.press(4_000, 2, Button::A);
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!(show.game.due_command(60_000), None);
    assert_eq!(show.game.view(60_000).auto_step, None);
    assert!(!show.game.controls().pause_clock);
}

#[test]
fn once_everyone_answered_the_answer_is_shown_after_the_wait() {
    let mut show = running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 3));
    show.press(3_000, 1, Button::B);
    assert_eq!(show.game.due_command(3_500), None, "one player has yet to answer");
    show.press(4_000, 2, Button::A);
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!(show.game.view(5_000).auto_step, Some(AutoStepView { action: AutoStep::Reveal, in_ms: 2_000 }));
    assert!(show.game.controls().pause_clock, "the quizmaster can hold it");
    assert_eq!(show.game.due_command(6_999), None);
    assert_eq!(show.game.due_command(7_000), Some(Command::Next));
    run_due(&mut show, 7_000);
    assert_eq!(step(&show.game), QuestionStep::Revealed { winner: None });
    assert_eq!(show.game.due_command(60_000), None, "the next slide waits for the quizmaster");
    assert_eq!(show.game.view(60_000).auto_step, None);

    // The step is journaled, so the show replays to the same place.
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    assert_eq!(replayed.phase(), show.game.phase());
}

#[test]
fn once_everyone_answered_the_show_goes_on_without_the_answer() {
    let slides = vec![question("q1", AnswerMode::Everyone), question("q2", AnswerMode::Everyone)];
    let mut show = running(show_with(slides, AllAnswered::NextSlide, 0));
    show.press(3_000, 1, Button::B);
    show.press(4_000, 2, Button::A);
    let scores = (show.score(1), show.score(2));
    assert_eq!(show.game.due_command(4_000), Some(Command::GoTo { slide: 1 }), "no wait");
    run_due(&mut show, 4_000);
    assert_eq!((show.game.slide_index(), step(&show.game)), (1, QuestionStep::Ready));
    assert_eq!((show.score(1), show.score(2)), scores, "the question was scored when it closed");

    // From the last slide, on to the final ranking as Next goes.
    show.run(5_000, Command::Next).unwrap();
    show.press(6_000, 1, Button::B);
    show.press(6_000, 2, Button::B);
    assert_eq!(show.game.due_command(6_000), Some(Command::GoToFinal));
    run_due(&mut show, 6_000);
    assert_eq!(show.game.phase(), &Phase::Final { step: 0 });
}

#[test]
fn the_quizmaster_can_step_in_or_hold_the_step_while_it_waits() {
    let answered = |show: &mut Show| {
        show.press(3_000, 1, Button::B);
        show.press(4_000, 2, Button::A);
    };
    // Next first: the answer shows now, and nothing more happens by itself.
    let mut show = running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::NextSlide, 5));
    answered(&mut show);
    show.run(5_000, Command::Next).unwrap();
    assert_eq!(step(&show.game), QuestionStep::Revealed { winner: None });
    assert_eq!(show.game.due_command(60_000), None);

    // Held: the question waits for Next, as without the setting.
    let mut show = running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 5));
    answered(&mut show);
    show.run(5_000, Command::PauseClock).unwrap();
    assert_eq!(show.game.due_command(60_000), None);
    assert_eq!(show.game.view(5_000).auto_step, None);
    assert!(!show.game.controls().pause_clock);
    assert_eq!(show.run(6_000, Command::PauseClock), Err(GameError::NotNow), "held once");
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    assert_eq!(replayed.due_command(60_000), None, "the hold is journaled");
}

#[test]
fn a_question_closed_by_time_up_or_by_the_quizmaster_waits_for_next() {
    let mut show = running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 0));
    show.press(3_000, 1, Button::B);
    run_due(&mut show, 12_000 + PRESS_GRACE_MS);
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!(show.game.due_command(60_000), None);

    let mut show = running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 0));
    show.run(3_000, Command::CloseQuestion).unwrap();
    assert_eq!(show.game.due_command(60_000), None);
}

#[test]
fn everyone_answered_counts_also_when_the_last_one_is_switched_off_or_all_buzzed_wrong() {
    let mut show = running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 1));
    show.press(3_000, 1, Button::B);
    show.run(4_000, Command::SetPlayerEnabled { buzzer: 2, enabled: false }).unwrap();
    assert_eq!(show.game.due_command(5_000), Some(Command::Next));

    let mut show = running(show_with(vec![question("q1", AnswerMode::FastestFinger)], AllAnswered::Reveal, 1));
    for (at, buzzer) in [(3_000, 1), (4_000, 2)] {
        show.press(at, buzzer, Button::Big);
        show.run(at + 500, Command::JudgeWrong).unwrap();
    }
    assert_eq!(step(&show.game), QuestionStep::Closed);
    assert_eq!(show.game.due_command(5_499), None);
    assert_eq!(show.game.due_command(5_500), Some(Command::Next));
}

/// `show` with every player having answered the running question by 4 s.
fn everyone_answered(mut show: Show) -> Show {
    show.press(3_000, 1, Button::B);
    show.press(4_000, 2, Button::A);
    show
}

#[test]
fn the_leaderboard_or_the_video_holds_the_step() {
    // The quizmaster opened the leaderboard: the step waits for their Next.
    let slides = vec![question("q1", AnswerMode::Everyone), question("q2", AnswerMode::Everyone)];
    let mut show = everyone_answered(running(show_with(slides, AllAnswered::NextSlide, 3)));
    show.run(5_000, Command::ShowLeaderboard { visible: true }).unwrap();
    assert_eq!(show.game.due_command(60_000), None);
    assert_eq!(show.game.view(5_000).auto_step, None);
    show.run(6_000, Command::ShowLeaderboard { visible: false }).unwrap();
    assert_eq!(show.game.due_command(60_000), None, "held until Next");

    // Hiding a leaderboard is no step of the quizmaster's.
    let mut show = everyone_answered(running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 3)));
    show.run(5_000, Command::ShowLeaderboard { visible: false }).unwrap();
    assert_eq!(show.game.due_command(7_000), Some(Command::Next));

    // The quizmaster played the question's video again.
    let mut show = everyone_answered(running(show_with(vec![with_video(question("q1", AnswerMode::Everyone))], AllAnswered::Reveal, 3)));
    show.run(5_000, Command::PlayVideo).unwrap();
    assert_eq!(show.game.due_command(60_000), None);
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    assert_eq!(replayed.due_command(60_000), None, "the hold replays");
}

#[test]
fn a_show_that_stopped_during_the_wait_holds_the_step_when_it_resumes() {
    let show = everyone_answered(running(show_with(vec![question("q1", AnswerMode::Everyone)], AllAnswered::Reveal, 3)));
    let mut crashed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    assert_eq!(crashed.due_command(60_000), Some(Command::Next), "overdue by the time the show is back");
    let command = crashed.recovery_command().expect("a pending step needs recovery");
    assert_eq!(command, Command::PauseClock);
    crashed.apply(&Stamped { at_ms: 60_000, command }).unwrap();
    assert_eq!(step(&crashed), QuestionStep::Closed);
    assert_eq!(crashed.due_command(60_000), None, "waits for the quizmaster's Next");
    assert_eq!(crashed.recovery_command(), None);
}

#[test]
fn enabling_players_who_are_on_starts_no_step() {
    let slides = vec![question("q1", AnswerMode::FastestFinger), question("q2", AnswerMode::Everyone)];
    let mut show = running(show_with(slides, AllAnswered::NextSlide, 0));
    for buzzer in 1..=2 {
        show.run(3_000, Command::SetPlayerEnabled { buzzer, enabled: true }).unwrap();
    }
    assert!(matches!(step(&show.game), QuestionStep::Running { .. }), "still open");
    assert_eq!(show.game.due_command(3_000), None);
    assert_eq!(show.game.view(3_000).auto_step, None);
}

/// Runs `command` and tells whether the show took a new step (`Game::step_no`).
fn takes_step(show: &mut Show, at_ms: u64, command: Command) -> bool {
    let before = show.game.step_no();
    show.run(at_ms, command).unwrap();
    show.game.step_no() != before
}

#[test]
fn the_show_counts_its_steps_as_the_quizmaster_sees_them() {
    let mut show = Show::new(vec![question("q1", AnswerMode::FastestFinger), question("q2", AnswerMode::Everyone)]);
    let press = |buzzer, button| Command::Press { buzzer, button };
    let steps = [
        (press(1, Button::Big), false), // registers in the lobby
        (press(2, Button::Big), false),
        (Command::Next, true), // the question
        (Command::Next, true), // its clock
        (Command::PauseClock, false),
        (Command::ResumeClock, false),
        (Command::ShowLeaderboard { visible: true }, false),
        (press(1, Button::Big), true), // buzzed in
        (Command::JudgeWrong, true),   // the clock runs on
        (press(2, Button::Big), true), // another buzzer buzzed in
        (Command::JudgeRight, true),
        (Command::Next, true), // the next question
        (Command::Next, true),
        (press(1, Button::B), false), // one player is still to answer
        (press(2, Button::A), true),  // closed: everyone answered
    ];
    for (n, (command, new_step)) in steps.into_iter().enumerate() {
        assert_eq!(takes_step(&mut show, 1_000 * n as u64, command.clone()), new_step, "{command:?}");
    }
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal, RULES_VERSION);
    assert_eq!(replayed.step_no(), show.game.step_no());
}
