import { useState } from "react";
import { api } from "../api";
import type { Translate } from "../i18n";
import type { Command, GameView } from "../types";

export function PlayersTable({ t, view, onError }: { t: Translate; view: GameView; onError: (m: string) => void }) {
  const run = (command: Command) => api.dispatch(command).catch((e) => onError(String(e)));
  const [newBuzzer, setNewBuzzer] = useState("");
  const players = [...view.players].sort((a, b) => a.buzzer - b.buzzer);

  const addPlayer = () => {
    const buzzer = Number(newBuzzer);
    if (!Number.isInteger(buzzer) || buzzer < 1) return;
    run({ type: "addPlayer", buzzer, name: "" });
    setNewBuzzer("");
  };

  return (
    <>
      <table className="players">
        <thead>
          <tr>
            <th>#</th>
            <th>{t("name")}</th>
            <th>{t("score")}</th>
            <th />
            <th>{t("active")}</th>
          </tr>
        </thead>
        <tbody>
          {players.map((p) => (
            <tr key={p.buzzer} className={[view.buzzed === p.buzzer ? "buzzed" : "", p.enabled ? "" : "disabled"].join(" ")}>
              <td>
                {p.buzzer}
                {p.answered && " ✓"}
                {p.lockedOut && " ✕"}
              </td>
              <td>
                <input
                  key={p.name}
                  defaultValue={p.name}
                  placeholder={t("buzzer", { n: p.buzzer })}
                  onBlur={(e) => e.target.value !== p.name && run({ type: "renamePlayer", buzzer: p.buzzer, name: e.target.value })}
                  onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                />
              </td>
              <td className="num">
                {p.score}
                {p.delta !== null && p.delta !== 0 && (
                  <span className={p.delta > 0 ? "good-text" : "bad-text"}> {p.delta > 0 ? `+${p.delta}` : p.delta}</span>
                )}
              </td>
              <td className="nowrap">
                {[-10, -1, 1, 10].map((delta) => (
                  <button key={delta} className="small" onClick={() => run({ type: "adjustScore", buzzer: p.buzzer, delta })}>
                    {delta > 0 ? `+${delta}` : delta}
                  </button>
                ))}
              </td>
              <td>
                <input
                  type="checkbox"
                  checked={p.enabled}
                  onChange={(e) => run({ type: "setPlayerEnabled", buzzer: p.buzzer, enabled: e.target.checked })}
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="row">
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
    </>
  );
}
