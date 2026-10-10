import { useState } from "react";
import { BUTTONS, buttonLabel } from "../answers";
import { useApp, useDispatch } from "../app-context";

/** On-screen buzzers for testing a quiz without hardware. */
export function VirtualBuzzers() {
  const { t } = useApp();
  const dispatch = useDispatch();
  const [count, setCount] = useState(6);

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
              <button key={button} className={`small key-${button}`} onClick={() => dispatch({ type: "press", buzzer, button })}>
                {buttonLabel(button)}
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
