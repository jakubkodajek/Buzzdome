import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { timeLeft } from "../answer-time";
import { ANSWER_LETTERS, answerIndex } from "../answers";
import { useApp } from "../app-context";
import { FINAL_LAST_STEP, FULL_SHARE, MAX_EVERYONE_ANSWERS } from "../bindings/Constants";
import { scoreDigits, signed } from "../format";
import { playerName } from "../i18n";
import { CheckIcon, CrossIcon, PeopleIcon, QuestionIcon, TrophyIcon } from "../icons";
import { rightAnswers, type QuestionPosition, type SlideContext } from "../quiz";
import { SlideMedia, type ScreenMode } from "../slide-media";
import { useTheme } from "../theme/context";
import { styleOf, textStyleCss, type StyledText } from "../theme/slide-style";
import type { AnswerResults, BuzzView, ClockView, GameView, PlayerView, QuestionSlide, Slide } from "../types";
import { podiumBlocks } from "./podium";
import { chartShares } from "./answer-chart";
import { LEADERBOARD_PAGE_MS, leaderboardPages, waitingStrip } from "./crowd";
import { useFitText } from "./fit-text";
import { Timer } from "./Timer";
import { WelcomeScreen } from "./WelcomeScreen";
import "./slide.css";

export function ranked(view: GameView): PlayerView[] {
  return view.players.filter((p) => p.rank !== null);
}

/**
 * What the projector shows for a game state. Fed only by its props, so the
 * quiz editor and the Director draw the same screens (see `ScreenMode`).
 * Outside the show, empty texts show what belongs there and the right
 * answer is ticked. Parts the editor can select carry `data-element`.
 */
export function SlideScreen(props: { view: GameView; context: SlideContext; clock: ClockView | null; mode?: ScreenMode }) {
  const { view, context, clock, mode = "show" } = props;
  const { t } = useApp();
  const phase = view.phase;
  const placeholder = (text: string, empty: string) => (text || mode === "show" ? text : <span className="placeholder">{empty}</span>);
  // A ranking's heading: the slide's title, else in the show the default one, as a placeholder elsewhere.
  const heading = (text: string, standard: string) => placeholder(text, standard) || standard;
  const look = (text: StyledText) => styledText(view.slide, text);
  switch (phase.phase) {
    case "welcome":
      return <WelcomeScreen view={view} mode={mode} />;
    case "lobby":
      return <Registration view={view} />;
    case "billboard":
      return view.slide?.type === "billboard" ? (
        <div className="center-slide">
          <h1 className="slide-title" data-element="title" style={look("title")}>
            {placeholder(view.slide.title, t("titlePlaceholder"))}
          </h1>
          <SlideMedia slide={view.slide} className="slide-media" mode={mode} live={{ view, clock }} />
          {(view.slide.text || mode === "preview") && (
            <p className="slide-text" data-element="text" style={look("text")}>
              {placeholder(view.slide.text, t("textPlaceholder"))}
            </p>
          )}
        </div>
      ) : null;
    case "explanation":
      return view.slide?.type === "explanation" ? (
        <div className="explanation-screen">
          {context.explains ? (
            <ExplainedHeader explained={context.explains.question} n={context.explains.position.n} />
          ) : (
            mode !== "show" && <p className="explained-missing">{t("noQuestionToExplain")}</p>
          )}
          <SlideMedia slide={view.slide} className="explanation-media" mode={mode} live={{ view, clock }} />
          <p className="explanation-text" data-element="text" style={look("text")}>
            {placeholder(view.slide.text, t("explanationPlaceholder"))}
          </p>
        </div>
      ) : null;
    case "endOfRound":
      return (
        <div className="center-slide">
          <h1 className="slide-title small" data-element="title" style={view.slide?.type === "endOfRound" ? look("title") : undefined}>
            {view.slide?.type === "endOfRound" ? heading(view.slide.title, t("endOfRound")) : t("endOfRound")}
          </h1>
          <Leaderboard players={ranked(view)} />
        </div>
      );
    case "final":
      return (
        <FinalScores
          players={ranked(view)}
          step={phase.step}
          title={view.slide?.type === "final" ? heading(view.slide.title, t("final")) : t("final")}
          titled={view.slide?.type === "final"}
          titleStyle={view.slide?.type === "final" ? look("title") : {}}
          sample={mode !== "show"}
        />
      );
    case "question":
      return view.slide?.type === "question" ? (
        <QuestionScreen
          view={view}
          slide={view.slide}
          clock={clock}
          position={context.position}
          mode={mode}
          placeholder={placeholder}
        />
      ) : null;
  }
}

/** The inline style of a slide's text from the slide's style (theme/slide-style.ts). */
function styledText(slide: Slide | null, text: StyledText): CSSProperties {
  return textStyleCss(styleOf(slide?.style, text), text);
}

/** The most buzzer numbers registration shows, and the most it shows in cells big enough to read from the room. */
const MAX_REGISTER_CELLS = 100;
const ROOMY_REGISTER_CELLS = 50;

/** Registration: every buzzer number, lit once it is pressed, with its team's name once paired. */
function Registration({ view }: { view: GameView }) {
  const { t } = useApp();
  const names = new Map(view.players.map((p) => [p.buzzer, p.name]));
  const highest = Math.max(20, ...view.players.map((p) => p.buzzer));
  const cells = Array.from({ length: Math.min(highest, MAX_REGISTER_CELLS) }, (_, i) => i + 1);
  return (
    <div className="welcome">
      <header className="welcome-header">{view.quizTitle}</header>
      <div className="welcome-body">
        <p className="slide-text">{t("lobbyHint")}</p>
        <div className={cells.length > ROOMY_REGISTER_CELLS ? "register-grid dense" : "register-grid"}>
          {cells.map((n) => (
            <span key={n} className={names.has(n) ? "on" : ""}>
              {n}
              {names.get(n) && <small>{names.get(n)}</small>}
            </span>
          ))}
        </div>
      </div>
      <footer className="welcome-footer">{t("registered", { n: view.players.length })}</footer>
    </div>
  );
}

/** The question an explanation slide explains, with its right answers. */
function ExplainedHeader({ explained, n }: { explained: QuestionSlide; n: number }) {
  const { t } = useApp();
  const correct = rightAnswers(explained);
  return (
    <header className="explained">
      <span className="explained-number">{t("questionNumber", { n })}</span>
      <h2 className="explained-question">{explained.text}</h2>
      {correct.length > 0 && (
        <div className={correct.length > 1 ? "explained-answers several" : "explained-answers"}>
          {correct.map(({ answer, share }) => (
            <div key={answer} className={`answer answer-${answer} is-correct explained-answer`}>
              <span className="letter">{ANSWER_LETTERS[answer]}</span>
              <span className="answer-text">{explained.answers[answer]}</span>
              <CorrectMark share={share} />
            </div>
          ))}
        </div>
      )}
    </header>
  );
}

/** The mark on a right answer: a tick, and the share of the points when it gives only part of them. */
function CorrectMark({ share }: { share: number }) {
  const { t } = useApp();
  return (
    <>
      {share < FULL_SHARE && <span className="answer-share digits">{t("percent", { n: share })}</span>}
      <CheckIcon className="answer-check" />
    </>
  );
}

function QuestionScreen(props: {
  view: GameView;
  slide: QuestionSlide;
  clock: ClockView | null;
  position: QuestionPosition | null;
  mode: ScreenMode;
  placeholder: (text: string, empty: string) => ReactNode;
}) {
  const { view, slide: question, clock, position, mode, placeholder } = props;
  const { t } = useApp();
  const step = view.phase.phase === "question" ? view.phase.step : "ready";
  const closed = step === "closed" || step === "revealed";
  const remaining = clock?.remainingMs ?? null;
  const enabled = view.players.filter((p) => p.enabled);
  const waiting = enabled.filter((p) => !p.answered);
  // In the show the quiz's settings can keep the answer and a buzz's points off the screen (spec QZ-11);
  // a hidden answer hides the counts that would give it away too.
  const show = mode === "show";
  const hidesAnswer = show && view.settings.hideAnswer;
  const screen = useRef<HTMLDivElement>(null);
  const themeId = useTheme()?.id ?? null;
  useFitText(screen, ".answer", JSON.stringify([question.text, question.answers, question.style ?? null, question.picture, question.video?.file ?? null, themeId]));

  let middle: ReactNode = null;
  if (view.results) {
    if (!hidesAnswer) middle = <ResultBars results={view.results} />;
  } else if (question.mode === "everyone" && !closed) {
    middle = <Waiting buzzers={waiting.map((p) => p.buzzer)} />;
  }

  const buzzed = view.players.find((p) => p.buzzer === view.buzzed);
  const winnerId = view.phase.phase === "question" && view.phase.step === "revealed" ? view.phase.winner : null;
  const winner = view.players.find((p) => p.buzzer === winnerId);
  const barPlayer = buzzed ?? winner;
  const buzzedAnswer = view.phase.phase === "question" && view.phase.step === "buzzed" ? answerIndex(view.phase.button) : null;
  const hasMedia = question.picture !== null || question.video !== null;
  const answerStyle = styledText(question, "answer");
  const marksAnswer = step === "revealed" && !hidesAnswer;
  const chart = chartShares(view);
  const buzzPoints = barPlayer?.delta ?? 0;
  const jokers = enabled.filter((p) => p.joker.on);
  const buzzBar = question.mode === "fastestFinger" && step !== "ready";
  // Several buzzes in the question: the bar shows them all in the order they came (rules version 8 queues them).
  const turns = question.mode === "fastestFinger" && (step === "running" || step === "buzzed" || barPlayer) ? view.buzzes : [];
  const queue = turns.length > 1 || (turns.length === 1 && !barPlayer);

  return (
    <div ref={screen} className={hasMedia ? "question-screen has-media" : "question-screen"}>
      <div className="info-panel">
        <Timer
          design={question.style?.timer}
          seconds={step === "ready" ? question.timeLimitS || null : closed ? 0 : remaining === null ? null : Math.ceil(remaining / 1000)}
          fraction={timeLeft(view.phase, clock)}
          counting={clock?.running ?? false}
          label={position ? `${position.n}/${position.total}` : ""}
        />
        <div className="info-middle">{middle}</div>
        {question.mode === "everyone" && (
          <div className="info-stat">
            <PeopleIcon />
            <span className="digits">{view.answeredCount}</span>
          </div>
        )}
        <div className="info-stat">
          <TrophyIcon />
          <span className="digits">{clock?.points ?? question.points}</span>
        </div>
      </div>

      <h1 className="question-text" data-element="question" style={styledText(question, "question")}>
        {placeholder(question.text, t("questionPlaceholder"))}
      </h1>
      {hasMedia && <SlideMedia slide={question} className="question-media" mode={mode} live={{ view, clock }} />}

      {question.answers.length > 0 && (
        <div className={question.answers.length > MAX_EVERYONE_ANSWERS ? "answers answers-6" : "answers"}>
          {question.answers.map((answer, i) => {
            const share = view.answerShares[i] ?? null;
            const isCorrect = share !== null;
            return (
              <div
                key={i}
                className={["answer", `answer-${i}`, marksAnswer ? (isCorrect ? "is-correct" : "is-dim") : "", chart ? "has-chart" : ""].join(" ")}
                data-element={`answer-${i}`}
                style={answerStyle}
              >
                <span className="letter">{ANSWER_LETTERS[i]}</span>
                <span className="answer-text">{placeholder(answer, t("answerPlaceholder", { letter: ANSWER_LETTERS[i] }))}</span>
                {closed && view.answerCounts && !hidesAnswer && <span className="answer-count">{view.answerCounts[i]}</span>}
                {chart && <span className="answer-bar" style={{ "--share": `${chart[i] * 100}%` } as CSSProperties} />}
                {(marksAnswer || !show) && isCorrect && <CorrectMark share={share} />}
              </div>
            );
          })}
        </div>
      )}

      {queue && (
        <div className="buzz-bar buzz-queue">
          <ol className="buzz-turns">
            {turns.map((turn) => (
              <BuzzTurn key={turn.buzzer} turn={turn} view={view} />
            ))}
          </ol>
          {barPlayer?.joker.on && <span className="buzz-joker">{t("jokerTimes")}</span>}
          {buzzPoints !== 0 && !(show && view.settings.hideBuzzPoints) && (
            <span className={`buzz-delta ${buzzPoints > 0 ? "plus" : "minus"}`}>{signed(buzzPoints)}</span>
          )}
        </div>
      )}
      {question.mode === "fastestFinger" && barPlayer && !queue && (
        <div className="buzz-bar">
          <span className="buzz-name">{playerName(t, barPlayer)}</span>
          {buzzedAnswer !== null && <span className={`buzz-answer answer-${buzzedAnswer}`}>{ANSWER_LETTERS[buzzedAnswer]}</span>}
          {barPlayer.joker.on && <span className="buzz-joker">{t("jokerTimes")}</span>}
          {buzzPoints !== 0 && !(show && view.settings.hideBuzzPoints) && (
            <span className={`buzz-delta ${buzzPoints > 0 ? "plus" : "minus"}`}>{signed(buzzPoints)}</span>
          )}
        </div>
      )}
      {question.mode === "fastestFinger" && step === "running" && !barPlayer && !queue && <div className="buzz-bar idle">{t("waitingForBuzz")}</div>}
      {question.mode === "fastestFinger" && step === "closed" && remaining === 0 && <div className="buzz-bar idle">{t("timeUp")}</div>}
      {!buzzBar && jokers.length > 0 && (
        <div className="joker-strip">
          <span className="joker-label">{t("jokerStrip")}</span>
          <span className="joker-names">{jokers.map((p) => playerName(t, p)).join(", ")}</span>
        </div>
      )}
    </div>
  );
}

/** The buzzers still to answer, in the info bar: in more rows as more wait, the last cell counting those it has no room for. */
function Waiting({ buzzers }: { buzzers: number[] }) {
  const { t } = useApp();
  const { rows, shown, more } = waitingStrip(buzzers.length);
  return (
    <div className={`responders rows-${rows}`}>
      {buzzers.slice(0, shown).map((buzzer) => (
        <span key={buzzer}>{buzzer}</span>
      ))}
      {more > 0 && <span className="more">{t("respondersMore", { n: more })}</span>}
    </div>
  );
}

/**
 * One buzz in the projector's queue: the team's name, crossed out once
 * judged wrong, ticked once right, lit while it answers. Its answer letter
 * shows once its turn came, never while it waits.
 */
function BuzzTurn({ turn, view }: { turn: BuzzView; view: GameView }) {
  const { t } = useApp();
  const player = view.players.find((p) => p.buzzer === turn.buzzer);
  const letter = turn.state === "waiting" ? null : answerIndex(turn.button);
  return (
    <li className={`buzz-turn ${turn.state}`}>
      <span className="buzz-turn-name">{player ? playerName(t, player) : turn.buzzer}</span>
      {letter !== null && <span className={`buzz-turn-answer answer-${letter}`}>{ANSWER_LETTERS[letter]}</span>}
      {turn.state === "wrong" && <CrossIcon className="buzz-turn-mark" />}
      {turn.state === "right" && <CheckIcon className="buzz-turn-mark" />}
    </li>
  );
}

function ResultBars({ results }: { results: AnswerResults }) {
  const { right, wrong, none } = results;
  const total = right + wrong + none;
  const width = (n: number) => `${total > 0 ? (n / total) * 100 : 0}%`;
  return (
    <div className="result-bars">
      <div className="result-row right">
        <CheckIcon />
        <span className="bar" style={{ width: width(right) }} />
        <span className="n">{right}</span>
      </div>
      <div className="result-row wrong">
        <CrossIcon />
        <span className="bar" style={{ width: width(wrong) }} />
        <span className="n">{wrong}</span>
      </div>
      <div className="result-row none">
        <QuestionIcon />
        <span className="bar" style={{ width: width(none) }} />
        <span className="n">{none}</span>
      </div>
    </div>
  );
}

/**
 * The ranking in columns of ten. When the teams take more than one page,
 * the pages take turns, each with the places it shows under it, so every
 * team finds its place at the size the room reads.
 */
export function Leaderboard({ players }: { players: PlayerView[] }) {
  const { t } = useApp();
  const pages = leaderboardPages(players);
  const page = usePageTurns(pages.length);
  const columns = pages[page] ?? [];
  const first = pages.slice(0, page).flat(2).length;
  const count = columns.flat().length;
  return (
    <div className={pages.length > 1 ? "leaderboard paged" : "leaderboard"}>
      {/* A new element for each page, so its rows come in afresh. */}
      <div key={page} className="leaderboard-columns">
        {columns.map((column, c) => (
          <ol key={c}>
            {column.map((p) => (
              <li key={p.buzzer} className={p.rank !== null && p.rank <= 3 ? `top top-${p.rank}` : ""}>
                <span className="rank">{p.rank}</span>
                <span className="name">{playerName(t, p)}</span>
                <span className="score">{scoreDigits(p.score)}</span>
              </li>
            ))}
          </ol>
        ))}
      </div>
      {pages.length > 1 && <p className="leaderboard-page">{t("leaderboardPlaces", { from: first + 1, to: first + count, total: players.length })}</p>}
    </div>
  );
}

/** The page shown of `count`, the next one every `LEADERBOARD_PAGE_MS`, round and round. */
function usePageTurns(count: number): number {
  const [turn, setTurn] = useState(0);
  useEffect(() => {
    if (count <= 1) return;
    const timer = setInterval(() => setTurn((current) => current + 1), LEADERBOARD_PAGE_MS);
    return () => clearInterval(timer);
  }, [count]);
  return count > 0 ? turn % count : 0;
}

/** Characters from which a name on the podium is drawn smaller, so it fits its block in two lines. */
const PODIUM_LONG_NAME = 18;

interface FinalScoresProps {
  players: PlayerView[];
  step: number;
  title: ReactNode;
  /** The title belongs to a final slide, which the editor can select. */
  titled: boolean;
  /** The final slide's style for its title. */
  titleStyle: CSSProperties;
  /** Outside the show: empty places show where the names go. */
  sample: boolean;
}

function FinalScores({ players, step, title, titled, titleStyle, sample }: FinalScoresProps) {
  const { t } = useApp();
  const heading = (text: ReactNode) => (
    <h1 className="slide-title small" data-element={titled ? "title" : undefined} style={titleStyle}>
      {text}
    </h1>
  );
  if (step >= FINAL_LAST_STEP) {
    return (
      <div className="center-slide">
        {heading(title)}
        <Leaderboard players={players} />
      </div>
    );
  }
  // Steps 1..3 reveal third, second, then first place on a podium. Players
  // who share a place stand on its block with the score they share.
  return (
    <div className="center-slide">
      {heading(step === 0 ? t("drumroll") : title)}
      <div className="podium">
        {podiumBlocks(players).map(({ place, named, more }) => {
          const shown = step >= FINAL_LAST_STEP - place;
          const first: PlayerView | undefined = named[0];
          const lines = named.length + (more > 0 ? 1 : 0);
          const long = named.some((p) => playerName(t, p).length > PODIUM_LONG_NAME);
          return (
            <div key={place} className={`podium-step place-${place}`}>
              <div className={`podium-name lines-${lines} ${long ? "long" : ""} ${shown && (first || sample) ? "shown" : ""}`}>
                {shown && first && (
                  <>
                    {named.map((p) => (
                      <span key={p.buzzer} className="name">
                        {playerName(t, p)}
                      </span>
                    ))}
                    {more > 0 && <span className="name">{t("podiumMore", { n: more })}</span>}
                    <span className="score">{scoreDigits(first.score)}</span>
                  </>
                )}
                {shown && !first && sample && <span className="name placeholder">{t("place", { n: place })}</span>}
              </div>
              <div className="podium-block">{place}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
