import { describe, expect, it } from "vitest";
import { FINAL_LAST_STEP } from "./bindings/Constants";
import { NO_CONTROLS, previewView } from "./host/preview";
import { translator } from "./i18n";
import { currentSlide, judgedLabel, nextLabel, phaseLabel } from "./phase";
import type { BuzzView, GameView, NextStep, Phase, PlayerView } from "./types";

const t = translator("en");
const slide = { id: "b1", type: "billboard", title: "Round 1", text: "", notes: "", picture: null, video: null, sound: null, background: null } as const;

function player(buzzer: number, name: string): PlayerView {
  return {
    buzzer,
    name,
    score: 0,
    enabled: true,
    totalAnswerMs: 0,
    rank: null,
    answered: false,
    answer: null,
    answerCorrect: null,
    answerMs: null,
    lockedOut: false,
    joinsNext: false,
    delta: null,
    joker: { on: false, chosen: false, left: 0, canChange: false },
  };
}

/** A show of five slides on slide `slideIndex`, in `phase`. */
function viewAt(phase: Phase, slideIndex = 1, more: Partial<GameView> = {}): GameView {
  return { ...previewView({ ...slide }, slideIndex, 5, "Quiz"), phase, ...more };
}

const clock = { limitMs: 20000, elapsedBeforeMs: 0, runningSinceMs: 0, pressesFromMs: 0 };

describe("currentSlide", () => {
  it("is the slide on the projector", () => {
    expect(currentSlide(viewAt({ phase: "billboard" }, 2))).toBe(2);
    expect(currentSlide(viewAt({ phase: "question", step: "ready" }, 0))).toBe(0);
  });

  it("is none before the slides and in the ranking after the last one", () => {
    expect(currentSlide(viewAt({ phase: "welcome" }, 0))).toBeNull();
    expect(currentSlide(viewAt({ phase: "lobby" }, 0))).toBeNull();
    expect(currentSlide(viewAt({ phase: "final", step: 0 }, 5))).toBeNull();
    // A final slide of the quiz itself is on the projector.
    expect(currentSlide(viewAt({ phase: "final", step: 0 }, 4))).toBe(4);
  });
});

describe("judgedLabel", () => {
  const players = [player(2, "Owls"), player(4, ""), player(7, "Brainiacs")];
  const buzzed = (buzzer: number, button: "big" | "b", buzzes: BuzzView[] = []) =>
    viewAt({ phase: "question", step: "buzzed", buzzer, button, clock }, 1, { players, buzzes });

  it("is nobody unless a player is being judged", () => {
    expect(judgedLabel(t, viewAt({ phase: "question", step: "running", clock }))).toBeNull();
    expect(judgedLabel(t, viewAt({ phase: "billboard" }))).toBeNull();
  });

  it("names the player, the answer they buzzed with, and the buzzer of a named player", () => {
    expect(judgedLabel(t, buzzed(2, "big"))).toEqual({ who: t("buzzed", { name: "Owls" }), detail: t("buzzer", { n: 2 }) });
    expect(judgedLabel(t, buzzed(2, "b"))).toEqual({ who: t("buzzedAnswer", { name: "Owls", letter: "B" }), detail: t("buzzer", { n: 2 }) });
    // A player without a name is called by their buzzer already.
    expect(judgedLabel(t, buzzed(4, "big"))).toEqual({ who: t("buzzed", { name: t("buzzer", { n: 4 }) }), detail: null });
  });

  it("puts the buzzes waiting their turn first in the detail, as Wrong hands the turn on to them", () => {
    const buzzes: BuzzView[] = [
      { buzzer: 2, button: "big", state: "wrong" },
      { buzzer: 7, button: "big", state: "answering" },
      { buzzer: 4, button: "big", state: "waiting" },
    ];
    expect(judgedLabel(t, buzzed(7, "big", buzzes))?.detail).toBe(`${t("buzzesWaiting", { n: 1 })} · ${t("buzzer", { n: 7 })}`);
  });

  it("is the status bar's phase in one line, without the buzzer", () => {
    const buzzes: BuzzView[] = [{ buzzer: 4, button: "big", state: "waiting" }];
    expect(phaseLabel(t, buzzed(2, "big", buzzes))).toBe(`${t("buzzed", { name: "Owls" })} · ${t("buzzesWaiting", { n: 1 })}`);
    expect(phaseLabel(t, buzzed(2, "big"))).toBe(t("buzzed", { name: "Owls" }));
  });
});

describe("nextLabel", () => {
  const withNext = (view: GameView, nextStep: NextStep | null, nextSlide: number | null = 2): GameView => ({
    ...view,
    nextSlide,
    controls: { ...NO_CONTROLS, next: nextStep !== null, nextStep },
  });

  it("says what Next does in the step", () => {
    expect(nextLabel(t, withNext(viewAt({ phase: "welcome" }, 0), "registration"))).toBe(t("nextRegistration"));
    expect(nextLabel(t, withNext(viewAt({ phase: "question", step: "ready" }), "startQuestion"))).toBe(t("nextStartQuestion"));
    expect(nextLabel(t, withNext(viewAt({ phase: "question", step: "closed" }), "showAnswer"))).toBe(t("nextShowAnswer"));
    expect(nextLabel(t, withNext(viewAt({ phase: "billboard" }), "nextSlide"))).toBe(t("nextSlide"));
    expect(nextLabel(t, withNext(viewAt({ phase: "billboard" }), null))).toBe(t("next"));
  });

  it("does not repeat End question's label while a question runs", () => {
    expect(nextLabel(t, withNext(viewAt({ phase: "question", step: "running", clock }), "endQuestion"))).not.toBe(t("endQuestion"));
  });

  it("starts the quiz from the registration and opens the final ranking from the last slide", () => {
    expect(nextLabel(t, withNext(viewAt({ phase: "lobby" }, 0), "nextSlide", 0))).toBe(t("nextStartQuiz"));
    expect(nextLabel(t, withNext(viewAt({ phase: "endOfRound" }, 4), "nextSlide", 5))).toBe(t("nextFinalRanking"));
  });

  it("names the place each step of the final ranking shows, the winner, then the whole ranking", () => {
    const final = (step: number) => nextLabel(t, withNext(viewAt({ phase: "final", step }, 5), "finalStep", null));
    expect(FINAL_LAST_STEP).toBe(4);
    expect(final(0)).toBe(t("nextFinalPlace", { n: 3 }));
    expect(final(1)).toBe(t("nextFinalPlace", { n: 2 }));
    expect(final(2)).toBe(t("nextFinalWinner"));
    expect(final(3)).toBe(t("nextFinalTable"));
  });
});
