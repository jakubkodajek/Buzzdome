import { useState } from "react";
import { buttonLabel } from "../answers";
import { useApp, useDispatch } from "../app-context";
import { MAX_NAME_CHARS } from "../bindings/Constants";
import { seconds, signed } from "../format";
import { usePreparedTeams, useTeamsLost } from "../hooks";
import { settingsLostText } from "../messages";
import type { BuzzerId, Command, GameView, SettingsLost } from "../types";
import { BuzzerChip, TeamList } from "./TeamList";

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
    <p className="teams-lost">
      {t("teamsLost")} {settingsLostText(t, lost)}
    </p>
  );
}

/**
 * Players with scores; select rows (Ctrl/Shift click) to change several at
 * once. Beside them, the team names the buzzers are paired with.
 */
function ShowTeams({ view, lost }: { view: GameView; lost: SettingsLost | null }) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const [selected, setSelected] = useState<Set<BuzzerId>>(new Set());
  const [anchor, setAnchor] = useState<BuzzerId | null>(null);
  const [newBuzzer, setNewBuzzer] = useState("");
  /** The buzzer clicked for pairing; clicking it again lets go of it. */
  const [picked, setPicked] = useState<BuzzerId | null>(null);
  const pick = (buzzer: BuzzerId | null) => setPicked((current) => (buzzer === current ? null : buzzer));
  const players = [...view.players].sort((a, b) => a.buzzer - b.buzzer);

  const select = (buzzer: BuzzerId, e: React.MouseEvent) => {
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null) {
      const ids = players.map((p) => p.buzzer);
      const [from, to] = [ids.indexOf(anchor), ids.indexOf(buzzer)].sort((a, b) => a - b);
      ids.slice(from, to + 1).forEach((id) => next.add(id));
    } else if (next.has(buzzer)) {
      next.delete(buzzer);
    } else {
      next.add(buzzer);
      setAnchor(buzzer);
    }
    setSelected(next);
  };

  const forSelected = (make: (buzzer: BuzzerId) => Command) => selected.forEach((buzzer) => dispatch(make(buzzer)));

  const addPlayer = () => {
    const buzzer = Number(newBuzzer);
    if (!Number.isInteger(buzzer) || buzzer < 1) return;
    dispatch({ type: "addPlayer", buzzer, name: "" });
    setNewBuzzer("");
  };

  return (
    <div className="teams-tab">
      <TeamsLost lost={lost} />
      <div className="toolbar">
        <span className="muted">{t("selectedPlayers", { n: selected.size })}</span>
        {[-10, -1, 1, 10].map((delta) => (
          <button key={delta} disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "adjustScore", buzzer, delta }))}>
            {signed(delta)}
          </button>
        ))}
        <button disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "setPlayerEnabled", buzzer, enabled: false }))}>
          {t("disable")}
        </button>
        <button disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "setPlayerEnabled", buzzer, enabled: true }))}>
          {t("enable")}
        </button>
        <span className="spacer" />
        <input
          type="number"
          min={1}
          value={newBuzzer}
          placeholder="#"
          className="narrow"
          onChange={(e) => setNewBuzzer(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addPlayer()}
        />
        <button onClick={addPlayer}>{t("addPlayer")}</button>
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
                    {p.answer ? buttonLabel(p.answer) : p.lockedOut ? "✕" : ""}
                    {p.answerMs !== null && <span className="muted"> · {seconds(p.answerMs)} s</span>}
                  </td>
                  <td className="num">{seconds(p.totalAnswerMs)} s</td>
                  <td>{p.enabled ? t("yes") : t("no")}</td>
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
