import { useState } from "react";
import { api } from "../api";
import type { Translate } from "../i18n";
import type { Button } from "../types";

const BUTTONS: Button[] = ["a", "b", "c", "d", "big"];

/** On-screen buzzers for testing a quiz without hardware. */
export function VirtualBuzzers({ t, onError }: { t: Translate; onError: (m: string) => void }) {
  const [count, setCount] = useState(6);
  const press = (buzzer: number, button: Button) =>
    api.dispatch({ type: "press", buzzer, button }).catch((e) => onError(String(e)));

  return (
    <div className="virtual-buzzers">
      <div className="row">
        <label className="inline">
          {t("count")}
          <input
            type="number"
            min={1}
            max={100}
            value={count}
            className="narrow"
            onChange={(e) => setCount(Math.max(1, Math.min(100, Number(e.target.value) || 1)))}
          />
        </label>
        <span className="muted">{t("keyboardHint")}</span>
      </div>
      <div className="buzzer-grid">
        {Array.from({ length: count }, (_, i) => i + 1).map((buzzer) => (
          <div key={buzzer} className="buzzer">
            <span className="buzzer-id">{buzzer}</span>
            {BUTTONS.map((button) => (
              <button key={button} className={`small key-${button}`} onClick={() => press(buzzer, button)}>
                {button === "big" ? "●" : button.toUpperCase()}
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
