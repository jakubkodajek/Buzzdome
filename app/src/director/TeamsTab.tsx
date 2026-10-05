import { useState } from "react";
import { api } from "../api";
import type { Translate } from "../i18n";
import type { Command, GameView } from "../types";

function seconds(ms: number): string {
  return (ms / 1000).toFixed(1);
}

/** Players with scores; select rows (Ctrl/Shift click) to change several at once. */
export function TeamsTab({ t, view, onError }: { t: Translate; view: GameView; onError: (m: string) => void }) {
  const run = (command: Command) => api.dispatch(command).catch((e) => onError(String(e)));
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [newBuzzer, setNewBuzzer] = useState("");
  const players = [...view.players].sort((a, b) => a.buzzer - b.buzzer);

  const select = (buzzer: number, e: React.MouseEvent) => {
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

  const forSelected = (make: (buzzer: number) => Command) => selected.forEach((buzzer) => run(make(buzzer)));

  const addPlayer = () => {
    const buzzer = Number(newBuzzer);
    if (!Number.isInteger(buzzer) || buzzer < 1) return;
    run({ type: "addPlayer", buzzer, name: "" });
    setNewBuzzer("");
  };

  return (
    <div className="teams-tab">
      <div className="toolbar">
        <span className="muted">{t("selectedPlayers", { n: selected.size })}</span>
        {[-10, -1, 1, 10].map((delta) => (
          <button key={delta} disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "adjustScore", buzzer, delta }))}>
            {delta > 0 ? `+${delta}` : delta}
          </button>
        ))}
        <button disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "setPlayerEnabled", buzzer, enabled: false }))}>
          {t("disable")}
        </button>
        <button disabled={selected.size === 0} onClick={() => forSelected((buzzer) => ({ type: "setPlayerEnabled", buzzer, enabled: true }))}>
          {t("enable")}
        </button>
        <span className="spacer" />
        <input type="number" min={1} value={newBuzzer} placeholder="#" className="narrow" onChange={(e) => setNewBuzzer(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addPlayer()} />
        <button onClick={addPlayer}>{t("addPlayer")}</button>
      </div>
      <div className="table-wrap">
        <table className="grid">
          <thead>
            <tr>
              <th>{t("name")}</th>
              <th>{t("colBuzzer")}</th>
              <th>{t("score")}</th>
              <th>{t("colLastResponse")}</th>
              <th>{t("colTotalTime")}</th>
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
                    placeholder={t("buzzer", { n: p.buzzer })}
                    onClick={(e) => e.stopPropagation()}
                    onBlur={(e) => e.target.value !== p.name && run({ type: "renamePlayer", buzzer: p.buzzer, name: e.target.value })}
                    onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                  />
                </td>
                <td className="num">{p.buzzer}</td>
                <td className="num strong">
                  {p.score}
                  {p.delta !== null && p.delta !== 0 && <span className={p.delta > 0 ? "good-text" : "bad-text"}> ({p.delta > 0 ? `+${p.delta}` : p.delta})</span>}
                </td>
                <td>
                  {p.answer ? p.answer.toUpperCase() : p.lockedOut ? "✕" : ""}
                  {p.answerMs !== null && <span className="muted"> · {seconds(p.answerMs)} s</span>}
                </td>
                <td className="num">{seconds(p.totalAnswerMs)} s</td>
                <td>{p.enabled ? t("yes") : t("no")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
