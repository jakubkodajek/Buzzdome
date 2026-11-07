import { useEffect, useState } from "react";
import { buttonLabel } from "../answers";
import { useApp, useDispatch, useRun } from "../app-context";
import { MAX_NAME_CHARS } from "../bindings/Constants";
import { confirmAction } from "../confirm";
import { seconds, signed } from "../format";
import { usePreparedTeams, useTeamsLost } from "../hooks";
import { playerName } from "../i18n";
import { CrossIcon } from "../icons";
import { isPlainKey } from "../keyboard";
import { settingsLostNotice } from "../messages";
import { NoticeBox } from "../notice";
import type { BuzzerId, Command, GameView, SettingsLost } from "../types";
import { BuzzerChip, TeamList } from "./TeamList";
import { clickSelection, typedBuzzer, typedDelta } from "./team-selection";

/**
 * In a show, its players and teams (`ShowTeams`). Before one, the team
 * names to prepare for it, which are kept from show to show.
 */
export function TeamsTab({ view }: { view: GameView | null }) {
  const prepared = usePreparedTeams();
  const lost = useTeamsLost();
  if (view) return <ShowTeams view={view} lost={lost} />;
  return (
    <div className="teams-tab">
      <TeamsLost lost={lost} />
      <div className="teams-body">
        <TeamList teams={prepared.map((name) => ({ name, buzzer: null }))} players={null} picked={null} onPick={() => {}} />
      </div>
    </div>
  );
}

/** Why the saved team names were not read when the app started, if so. */
function TeamsLost({ lost }: { lost: SettingsLost | null }) {
  const { t } = useApp();
  if (!lost) return null;
  return (
    <NoticeBox notice={settingsLostNotice(t, t("teamsLost"), lost)} />
  );
}

/**
 * Players with scores; select rows (Ctrl/Shift click) to change several at
 * once. Beside them, the team names the buzzers are paired with.
 */
function ShowTeams({ view, lost }: { view: GameView; lost: SettingsLost | null }) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const [selected, setSelected] = useState<Set<BuzzerId>>(new Set());
  const [anchor, setAnchor] = useState<BuzzerId | null>(null);
  const [newBuzzer, setNewBuzzer] = useState("");
  /** The buzzer clicked for pairing; clicking it again lets go of it. */
  const [picked, setPicked] = useState<BuzzerId | null>(null);
  const pick = (buzzer: BuzzerId | null) => setPicked((current) => (buzzer === current ? null : buzzer));
  const players = [...view.players].sort((a, b) => a.buzzer - b.buzzer);
  const jokers = view.settings.jokers > 0;

  const select = (buzzer: BuzzerId, e: React.MouseEvent) => {
    const keys = { toggle: e.ctrlKey || e.metaKey, range: e.shiftKey };
    setSelected(clickSelection(selected, players.map((p) => p.buzzer), buzzer, keys, anchor));
    if (!keys.range) setAnchor(buzzer);
  };
  // Esc lets go of the selection, so a later score change cannot go to a row picked long ago.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && isPlainKey(event)) setSelected(new Set());
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const forSelected = (make: (buzzer: BuzzerId) => Command) => selected.forEach((buzzer) => dispatch(make(buzzer)));
  const adjust = (delta: number) => forSelected((buzzer) => ({ type: "adjustScore", buzzer, delta }));
  // Any number of points at once, to put a wrong judgement right.
  const [deltaText, setDeltaText] = useState("");
  const delta = typedDelta(deltaText);
  const adjustTyped = () => {
    if (delta === null || selected.size === 0) return;
    adjust(delta);
    setDeltaText("");
  };
  // Refused mid-question like the jumps (`Controls.goTo`), and asks first.
  const resetScores = () => run(confirmAction(t, t("resetScoresConfirm")).then((yes) => yes && dispatch({ type: "resetScores" })));

  const added = typedBuzzer(newBuzzer);
  const addPlayer = () => {
    if (added === null) return;
    dispatch({ type: "addPlayer", buzzer: added, name: "" });
    setNewBuzzer("");
  };

  return (
    <div className="teams-tab">
      <TeamsLost lost={lost} />
      <div className="toolbar">
        <span className="muted">{t("selectedPlayers", { n: selected.size })}</span>
        <button onClick={() => setSelected(new Set(players.map((p) => p.buzzer)))}>{t("selectAllPlayers")}</button>
        {[-10, -1, 1, 10].map((step) => (
          <button key={step} disabled={selected.size === 0} onClick={() => adjust(step)}>
            {signed(step)}
          </button>
        ))}
        <input
          type="number"
          step={1}
          value={deltaText}
          className="narrow score-delta"
          placeholder={t("score")}
          title={t("scoreDelta")}
          aria-label={t("scoreDelta")}
          onChange={(e) => setDeltaText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && adjustTyped()}
        />
        <button disabled={selected.size === 0 || delta === null} title={t("scoreDelta")} onClick={adjustTyped}>
          {t("scoreDeltaApply")}
        </button>
        <button disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "setPlayerEnabled", buzzer, enabled: false }))}>
          {t("disable")}
        </button>
        <button disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "setPlayerEnabled", buzzer, enabled: true }))}>
          {t("enable")}
        </button>
        <span className="toolbar-separator" />
        <button disabled={!view.controls.goTo} onClick={resetScores}>
          {t("resetScores")}
        </button>
        <span className="spacer" />
        <input
          type="number"
          min={1}
          value={newBuzzer}
          placeholder={t("colBuzzer")}
          title={t("addPlayerHint")}
          aria-label={t("addPlayerHint")}
          className="narrow add-buzzer"
          onChange={(e) => setNewBuzzer(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addPlayer()}
        />
        <button disabled={added === null} onClick={addPlayer}>
          {t("addPlayer")}
        </button>
      </div>
      <div className="teams-body">
        <div className="table-wrap">
          <table className="grid">
            <thead>
              <tr>
                <th>{t("name")}</th>
                <th>{t("colBuzzer")}</th>
                <th>{t("score")}</th>
                <th>{t("colLastResponse")}</th>
                <th title={t("colTotalTimeHint")}>{t("colTotalTime")}</th>
                <th>{t("active")}</th>
                {jokers && <th title={t("colJokerHint")}>{t("colJoker")}</th>}
              </tr>
            </thead>
            <tbody>
              {players.map((p) => (
                <tr
                  key={p.buzzer}
                  className={[selected.has(p.buzzer) ? "selected" : "", view.buzzed === p.buzzer ? "current" : "", p.enabled ? "" : "disabled"].join(" ")}
                  onClick={(e) => select(p.buzzer, e)}
                >
                  <td>
                    <input
                      className="cell-input"
                      key={p.name}
                      defaultValue={p.name}
                      maxLength={MAX_NAME_CHARS}
                      placeholder={t("buzzer", { n: p.buzzer })}
                      onClick={(e) => e.stopPropagation()}
                      onBlur={(e) => e.target.value !== p.name && dispatch({ type: "renamePlayer", buzzer: p.buzzer, name: e.target.value })}
                      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
                    />
                  </td>
                  <td className="num">
                    <BuzzerChip buzzer={p.buzzer} free={!p.name} picked={picked === p.buzzer} onPick={pick} />
                  </td>
                  <td className="num strong">
                    {p.score}
                    {p.delta !== null && p.delta !== 0 && <span className={p.delta > 0 ? "good-text" : "bad-text"}> ({signed(p.delta)})</span>}
                  </td>
                  <td>
                    {p.answer ? buttonLabel(p.answer) : p.lockedOut ? <CrossIcon className="inline-icon" /> : p.joinsNext ? <span className="muted">{t("joinsNext")}</span> : ""}
                    {p.answerMs !== null && <span className="muted"> · {seconds(p.answerMs)} s</span>}
                  </td>
                  <td className="num">{seconds(p.totalAnswerMs)} s</td>
                  <td>{p.enabled ? t("yes") : t("no")}</td>
                  {jokers && (
                    <td className="joker-cell">
                      <button
                        className="joker-toggle"
                        aria-pressed={p.joker.chosen}
                        disabled={!p.joker.canChange}
                        title={t("jokerPlayFor", { name: playerName(t, p) })}
                        aria-label={t("jokerPlayFor", { name: playerName(t, p) })}
                        onClick={(e) => {
                          e.stopPropagation();
                          dispatch({ type: "setJoker", buzzer: p.buzzer, on: !p.joker.chosen });
                        }}
                      >
                        {t("jokerPlay")}
                      </button>
                      <span className="muted"> {t("jokersLeft", { n: p.joker.left })}</span>
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <TeamList teams={view.teams} players={view.players} picked={picked} onPick={pick} />
      </div>
    </div>
  );
}
