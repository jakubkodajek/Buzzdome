//! The order in which the show thread handles what happens (architecture
//! review N2): every receiver event already received comes before a
//! time-driven command, so "time is up" never beats a press that arrived in
//! time, however far behind the thread has fallen.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

/// Waits up to `wait` for an event, handles it and every event received
/// since, then runs `tick` (time up, clock updates). Returns false once the
/// sender is gone; `tick` still runs.
pub fn pump_once<E>(events: &Receiver<E>, wait: Duration, mut handle: impl FnMut(E), tick: impl FnOnce()) -> bool {
    let connected = match events.recv_timeout(wait) {
        Ok(event) => {
            handle(event);
            loop {
                match events.try_recv() {
                    Ok(event) => handle(event),
                    Err(TryRecvError::Empty) => break true,
                    Err(TryRecvError::Disconnected) => break false,
                }
            }
        }
        Err(RecvTimeoutError::Timeout) => true,
        Err(RecvTimeoutError::Disconnected) => false,
    };
    tick();
    connected
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{AnswerMode, Button, Command, CorrectAnswer, Game, Phase, Question, QuestionStep, Quiz, Slide, SlideContent, Stamped};
    use std::cell::RefCell;
    use std::sync::mpsc;

    fn everyone_question() -> Quiz {
        let question = Question {
            text: "Q?".into(),
            mode: AnswerMode::Everyone,
            answers: vec!["A".into(), "B".into(), "C".into(), "D".into()],
            correct: vec![CorrectAnswer::full(1)],
            time_limit_s: 10,
            points: 100,
            points_end: None,
            penalty: 0,
        };
        Quiz::new("T", vec![Slide::new("q", SlideContent::Question(question))])
    }

    #[test]
    fn presses_received_in_time_count_even_when_the_thread_is_late() {
        let mut game = Game::new(everyone_question());
        // 31 players, so the question stays open until time is up.
        for buzzer in 1..=31 {
            game.apply(&Stamped { at_ms: 0, command: Command::Press { buzzer, button: Button::Big } }).unwrap();
        }
        game.apply(&Stamped { at_ms: 0, command: Command::Next }).unwrap();
        game.apply(&Stamped { at_ms: 1_000, command: Command::Next }).unwrap();
        // Time is up at 11 s. Thirty presses came in during its last second, but
        // the thread only gets to them two seconds later.
        let (send, events) = mpsc::channel();
        for buzzer in 1..=30 {
            send.send(Stamped { at_ms: 10_000 + u64::from(buzzer) * 30, command: Command::Press { buzzer, button: Button::B } }).unwrap();
        }
        let late = 13_000;
        let game = RefCell::new(game);
        pump_once(
            &events,
            Duration::ZERO,
            |stamped| {
                game.borrow_mut().apply(&stamped).unwrap();
            },
            || {
                let due = game.borrow().due_command(late);
                if let Some(command) = due {
                    game.borrow_mut().apply(&Stamped { at_ms: late, command }).unwrap();
                }
            },
        );
        let game = game.into_inner();
        assert_eq!(game.phase(), &Phase::Question(QuestionStep::Closed));
        assert_eq!(game.responses().len(), 30);
    }

    #[test]
    fn every_queued_event_comes_before_the_tick() {
        let (send, events) = mpsc::channel();
        for n in 0..5 {
            send.send(n).unwrap();
        }
        let order = RefCell::new(Vec::new());
        let connected = pump_once(&events, Duration::from_millis(10), |n| order.borrow_mut().push(n.to_string()), || order.borrow_mut().push("tick".to_string()));
        assert!(connected);
        assert_eq!(order.into_inner(), ["0", "1", "2", "3", "4", "tick"]);
        drop(send);
        let mut ticked = false;
        assert!(!pump_once(&events, Duration::from_millis(10), |_: i32| {}, || ticked = true));
        assert!(ticked);
    }
}
