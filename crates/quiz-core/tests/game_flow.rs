use quiz_core::{
    AnswerMode, Button, Command, Cue, Effect, Game, GameError, Light, Phase, Question, QuestionStep, Quiz, Slide,
    SlideContent, Stamped, VideoClip, team_names, MAX_NAME_CHARS, MAX_TEAMS, PRESS_GRACE_MS,
};
use quiz_core::game::FINAL_LAST_STEP;
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
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal).view(0).teams, show.game.view(0).teams);
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
    let replayed = Game::replay(show.game.quiz().clone(), &show.journal);
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
        QuestionStep::Buzzed { buzzer: 1, clock } => assert_eq!(clock.elapsed_ms(20_000), 1_000),
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
    assert_eq!(Game::replay(show.game.quiz().clone(), &show.journal), show.game);

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
    assert_eq!(show.game.view(0).next_slide_title.as_deref(), Some("thanks"));
    for expected in 1..=4 {
        show.run(0, Command::Next).unwrap();
        assert_eq!(show.game.phase(), &Phase::Final { step: expected });
    }
    show.run(0, Command::Next).unwrap();
    assert_eq!(show.game.current_slide().map(|s| s.id.as_str()), Some("thanks"));
    // No second ranking after the last slide: the quiz has its own.
    assert!(!show.game.controls().next);
    assert_eq!(show.run(0, Command::Next), Err(GameError::NotNow));
    assert_eq!(show.game.view(0).next_slide_title, None);

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
