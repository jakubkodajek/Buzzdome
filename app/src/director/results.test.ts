import { describe, expect, it } from "vitest";
import { newSlide } from "../editor/slides";
import { translator } from "../i18n";
import type { AskedQuestion, Mark, Quiz, ShowResults, Slide, SlideType } from "../types";
import { marksByTime, questionLabels, resultRounds, resultsTable } from "./results";

const t = translator("en");

const slide = (type: SlideType, text = ""): Slide => {
  const made = newSlide(type, `s-${text || type}`);
  return made.type === "question" ? { ...made, text } : made;
};

// Questions 1 and 2 in round 1, question 3 in round 2.
const quiz: Quiz = {
  schemaVersion: 2,
  title: "Pub quiz",
  theme: null,
  slides: [slide("billboard"), slide("question", "Capital?"), slide("question", "Two plus two?"), slide("endOfRound"), slide("question", "Sky?")],
};

const right = (button: "a" | "b", elapsedMs: number, points: number, joker = false): Mark => ({ answer: { button, elapsedMs }, right: true, points, joker });
const noAnswer: Mark = { answer: null, right: false, points: -5, joker: false };

// Question 2 was skipped; the Owls played their joker on question 3.
const results: ShowResults = {
  questions: [
    {
      slide: 1,
      round: 0,
      marks: [
        [1, right("b", 2500, 90)],
        [2, noAnswer],
      ],
    },
    { slide: 4, round: 1, marks: [[2, right("a", 1200, 180, true)]] },
  ],
  rounds: 2,
  players: [
    { buzzer: 2, name: "Owls", rank: 1, score: 175, totalAnswerMs: 1200, rounds: [-5, 180], adjusted: 0 },
    { buzzer: 1, name: "", rank: 2, score: 100, totalAnswerMs: 2500, rounds: [90, 0], adjusted: 10 },
    { buzzer: 3, name: "", rank: null, score: 0, totalAnswerMs: 0, rounds: [0, 0], adjusted: 0 },
  ],
};

describe("questionLabels", () => {
  it("names a question by its number among the questions and its text", () => {
    const label = questionLabels(quiz);
    expect([label(1), label(2), label(4)]).toEqual(["1. Capital?", "2. Two plus two?", "3. Sky?"]);
  });
});

describe("marksByTime", () => {
  it("lists the fastest answer first and no answer last", () => {
    const question: AskedQuestion = {
      slide: 1,
      round: 0,
      marks: [
        [3, noAnswer],
        [2, right("a", 3000, 80)],
        [1, right("b", 900, 100)],
      ],
    };
    expect(marksByTime(question).map(([buzzer]) => buzzer)).toEqual([1, 2, 3]);
  });
});

describe("resultRounds", () => {
  it("adds up the rounds only when the quiz has more than one", () => {
    expect(resultRounds(results)).toEqual([0, 1]);
    expect(resultRounds({ ...results, rounds: 1 })).toEqual([]);
  });
});

describe("resultsTable", () => {
  it("has a row for each player in the ranking's order with what each question and round gave them", () => {
    expect(resultsTable(t, quiz, results, "comma")).toEqual([
      ["Place", "Buzzer", "Name", "1. Capital?", "3. Sky?", "Round 1", "Round 2", "Adjustments", "Score", "Total time (s)"],
      ["1", "2", "Owls", "-5", "180", "-5", "180", "0", "175", "1.20"],
      ["2", "1", "Buzzer 1", "90", "", "90", "0", "10", "100", "2.50"],
      ["", "3", "Buzzer 3", "", "", "0", "0", "0", "0", "0.00"],
    ]);
  });

  it("writes the decimal comma where the list separator is a semicolon, and no round columns for one round", () => {
    const table = resultsTable(t, quiz, { ...results, rounds: 1, players: results.players.slice(0, 1) }, "semicolon");
    expect(table).toEqual([
      ["Place", "Buzzer", "Name", "1. Capital?", "3. Sky?", "Adjustments", "Score", "Total time (s)"],
      ["1", "2", "Owls", "-5", "180", "0", "175", "1,20"],
    ]);
  });
});
