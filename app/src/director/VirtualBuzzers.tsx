import { useState } from "react";
import { BUTTONS, buttonLabel } from "../answers";
import { useApp, useDispatch } from "../app-context";
import { keyboardHelp } from "../keyboard";
import { BUZZER_COUNT, buzzerCount, readBuzzerCount, writeBuzzerCount } from "./virtual-buzzers";

/** On-screen buzzers for testing a quiz without hardware; their number is kept on this computer. */
export function VirtualBuzzers() {
  const { t } = useApp();
  const dispatch = useDispatch();
  const [count, setCountState] = useState(readBuzzerCount);
  // The field as typed: cleared to type another number, it changes nothing until it has one.
  const [typed, setTyped] = useState(String(count));
  const setCount = (text: string) => {
    setTyped(text);
    const next = buzzerCount(text, count);
    setCountState(next);
    writeBuzzerCount(next);
  };

  return (
    <div className="virtual-buzzers">
      <div className="row">
        <label className="inline">
          {t("count")}
          <input
            type="number"
            min={BUZZER_COUNT.min}
            max={BUZZER_COUNT.max}
            value={typed}
            className="narrow"
            onChange={(e) => setCount(e.target.value)}
            onBlur={() => setTyped(String(count))}
          />
        </label>
        <span className="muted">{keyboardHelp(t)}</span>
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
