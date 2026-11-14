import { useState } from "react";
import { BUTTONS, buttonName } from "../answers";
import { useApp, useDispatch } from "../app-context";
import { useGameView } from "../hooks";
import { BigButtonIcon } from "../icons";
import { keyboardHelp } from "../keyboard";
import { BUZZER_COUNT, buzzerCount, readBuzzerCount, readPickedBuzzerCount, writeBuzzerCount } from "./virtual-buzzers";

/** On-screen buzzers for testing a quiz without hardware; their number is kept on this computer. */
export function VirtualBuzzers() {
  const { t } = useApp();
  const dispatch = useDispatch();
  const registered = useGameView()?.players.length ?? 0;
  // The count the quizmaster picked (here or last time); none yet, the registered players' (the view may come after this opens).
  const [picked, setPicked] = useState<number | null>(readPickedBuzzerCount);
  const count = picked ?? readBuzzerCount(registered);
  // The field as typed: cleared to type another number, it changes nothing until it has one.
  const [typed, setTyped] = useState<string | null>(null);
  const setCount = (text: string) => {
    setTyped(text);
    const next = buzzerCount(text, count);
    setPicked(next);
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
            value={typed ?? String(count)}
            className="narrow"
            onChange={(e) => setCount(e.target.value)}
            onBlur={() => setTyped(null)}
          />
        </label>
        <span className="muted">{keyboardHelp(t)}</span>
      </div>
      <div className="buzzer-grid">
        {Array.from({ length: count }, (_, i) => i + 1).map((buzzer) => (
          <div key={buzzer} className="buzzer">
            <span className="buzzer-id">{buzzer}</span>
            {BUTTONS.map((button) => (
              <button
                key={button}
                className={`small key-${button}`}
                title={buttonName(t, button)}
                aria-label={buttonName(t, button)}
                onClick={() => dispatch({ type: "press", buzzer, button })}
              >
                {button === "big" ? <BigButtonIcon className="inline-icon" /> : buttonName(t, button)}
              </button>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
