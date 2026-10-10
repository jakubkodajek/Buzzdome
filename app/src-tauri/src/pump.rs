//! The order in which the show thread handles what happens (architecture
//! review N2): every event received by a moment comes before the
//! time-driven commands of that moment, so "time is up" never beats a press
//! that arrived in time, however far behind the thread has fallen. And a
//! burst of events never holds time up (A8): the thread takes the events
//! received up to the moment it reads the clock, then ticks at that moment,
//! and leaves the later ones for the next round.

use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

/// An event with the moment it happened on the show clock. One with no
/// moment of its own (a receiver status) says 0, so it never waits.
pub trait Timed {
    fn at_ms(&self) -> u64;
}

/// The show thread's queue of events.
pub struct Pump<E> {
    events: Receiver<E>,
    /// An event received after the clock was read, kept for the next round.
    held: Option<E>,
}

impl<E: Timed> Pump<E> {
    pub fn new(events: Receiver<E>) -> Pump<E> {
        Pump { events, held: None }
    }

    /// Waits up to `wait` for an event (not when one is held), reads the
    /// clock, handles every event received that happened by then, in the
    /// order received, then runs `tick` with the same moment (time up, clock
    /// updates). Returns false once the senders are gone; `tick` still runs.
    pub fn once(&mut self, wait: Duration, clock: impl FnOnce() -> u64, mut handle: impl FnMut(E), tick: impl FnOnce(u64)) -> bool {
        let mut connected = true;
        if self.held.is_none() {
            match self.events.recv_timeout(wait) {
                Ok(event) => self.held = Some(event),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => connected = false,
            }
        }
        let now = clock();
        while let Some(event) = self.held.take() {
            if event.at_ms() > now {
                self.held = Some(event);
                break;
            }
            handle(event);
            match self.events.try_recv() {
                Ok(event) => self.held = Some(event),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => connected = false,
            }
        }
        tick(now);
        connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{AnswerMode, Button, Command, CorrectAnswer, Game, Phase, Question, QuestionStep, Quiz, Slide, SlideContent, Stamped};
    use std::cell::{Cell, RefCell};
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

    impl Timed for Stamped {
        fn at_ms(&self) -> u64 {
            self.at_ms
        }
    }

    /// An event that only has a moment.
    #[derive(Debug, PartialEq)]
    struct At(u64);

    impl Timed for At {
        fn at_ms(&self) -> u64 {
            self.0
        }
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
        let game = RefCell::new(game);
        Pump::new(events).once(
            Duration::ZERO,
            || 13_000,
            |stamped| {
                game.borrow_mut().apply(&stamped).unwrap();
            },
            |now| {
                let due = game.borrow().due_command(now);
                if let Some(command) = due {
                    game.borrow_mut().apply(&Stamped { at_ms: now, command }).unwrap();
                }
            },
        );
        let game = game.into_inner();
        assert_eq!(game.phase(), &Phase::Question(QuestionStep::Closed));
        assert_eq!(game.responses().len(), 30);
    }

    #[test]
    fn every_event_received_by_the_moment_comes_before_its_tick() {
        let (send, events) = mpsc::channel();
        for n in [0, 3, 1, 10] {
            send.send(At(n)).unwrap();
        }
        let order = RefCell::new(Vec::new());
        let mut pump = Pump::new(events);
        let connected = pump.once(Duration::from_millis(10), || 10, |At(n)| order.borrow_mut().push(n.to_string()), |now| order.borrow_mut().push(format!("tick {now}")));
        assert!(connected);
        // In the order received, not re-ordered by their moments.
        assert_eq!(order.into_inner(), ["0", "3", "1", "10", "tick 10"]);
        drop(send);
        let mut ticked = false;
        assert!(!pump.once(Duration::from_millis(10), || 20, |_| {}, |_| ticked = true));
        assert!(ticked);
    }

    #[test]
    fn a_burst_that_keeps_coming_does_not_hold_time_up() {
        // Every event takes 10 ms to handle (a journal write), and meanwhile the next one comes in.
        let (send, events) = mpsc::channel();
        send.send(At(1_000)).unwrap();
        let clock = Cell::new(1_000);
        let order = RefCell::new(Vec::new());
        let mut pump = Pump::new(events);
        for _ in 0..3 {
            pump.once(
                Duration::ZERO,
                || clock.get(),
                |At(n)| {
                    order.borrow_mut().push(n.to_string());
                    clock.set(clock.get() + 10);
                    send.send(At(clock.get())).unwrap();
                },
                |now| order.borrow_mut().push(format!("tick {now}")),
            );
        }
        assert_eq!(order.into_inner(), ["1000", "tick 1000", "1010", "tick 1010", "1020", "tick 1020"]);
        // The event that came in last waits for the next round; none is lost.
        assert_eq!(pump.held, Some(At(1_030)));
    }
}
