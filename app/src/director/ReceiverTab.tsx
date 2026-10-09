import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { Translate } from "../i18n";
import type { Button, DeviceInfo, LearnProgress, Press, RawReport, ReceiverInfo, ReceiverStatus } from "../types";

const BUTTONS: Button[] = ["a", "b", "c", "d", "big"];
const LOG_LEN = 40;

const hex4 = (n: number) => n.toString(16).padStart(4, "0");
const vidPid = (d: { vendorId: number; productId: number }) => `${hex4(d.vendorId)}:${hex4(d.productId)}`;
const hexBytes = (bytes: number[]) => bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ");
const buttonLabel = (b: Button) => (b === "big" ? "●" : b.toUpperCase());
const pressLabel = (p: Press) => `${p.buzzer} ${buttonLabel(p.button)}`;
const isKeyboardOrMouse = (d: DeviceInfo) => d.usagePage === 1 && (d.usage === 2 || d.usage === 6);

function listenTo<T>(event: string, handler: (payload: T) => void): () => void {
  const unlisten = listen<T>(event, (e) => handler(e.payload));
  return () => {
    unlisten.then((stop) => stop());
  };
}

/** Receiver setup and diagnostics: pick the USB receiver, watch its raw data, teach buttons. */
export function ReceiverTab({ t, onError }: { t: Translate; onError: (m: string) => void }) {
  const [info, setInfo] = useState<ReceiverInfo | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [raw, setRaw] = useState<RawReport[]>([]);
  const [presses, setPresses] = useState<{ press: Press; used: boolean; at: number }[]>([]);
  const [learn, setLearn] = useState<LearnProgress | null>(null);
  const [buzzer, setBuzzer] = useState(1);
  const [button, setButton] = useState<Button>("a");
  const [message, setMessage] = useState<string | null>(null);
  const [udev, setUdev] = useState("");

  const reload = useCallback(() => {
    api
      .receiverInfo()
      .then((i) => {
        setInfo(i);
        setStatus(i.status);
        setLearn(i.learn);
        setRaw([...i.recent].reverse().slice(0, LOG_LEN));
      })
      .catch((e) => onError(String(e)));
  }, [onError]);

  const refreshDevices = useCallback(() => {
    api.receiverDevices().then(setDevices).catch((e) => onError(String(e)));
  }, [onError]);

  useEffect(() => {
    reload();
    refreshDevices();
    const offs = [
      listenTo<ReceiverStatus>("receiver-status", (s) => {
        setStatus(s);
        reload();
      }),
      listenTo<RawReport>("receiver-raw", (r) => setRaw((l) => [r, ...l].slice(0, LOG_LEN))),
      listenTo<{ press: Press; used: boolean }>("receiver-press", (p) =>
        setPresses((l) => [{ ...p, at: Date.now() }, ...l].slice(0, LOG_LEN)),
      ),
      listenTo<LearnProgress>("receiver-learn", setLearn),
    ];
    return () => offs.forEach((off) => off());
  }, [reload, refreshDevices]);

  useEffect(() => {
    if (info?.platform === "linux") api.receiverUdevRule().then(setUdev).catch(() => setUdev(""));
  }, [info?.platform, status]);

  const connected = status?.kind === "connected" ? status.device : null;
  const run = (p: Promise<unknown>) => p.catch((e) => onError(String(e)));

  const exportLog = async () => {
    const path = await save({ defaultPath: "receiver-log.txt", filters: [{ name: "Text", extensions: ["txt"] }] });
    if (path) run(api.receiverExportLog(path));
  };

  const finishLearn = async () => {
    try {
      setMessage(await api.receiverLearnFinish());
      // Move on to the next button, then the next buzzer.
      const i = BUTTONS.indexOf(button);
      if (i < BUTTONS.length - 1) setButton(BUTTONS[i + 1]);
      else {
        setButton("a");
        setBuzzer(buzzer + 1);
      }
      reload();
    } catch (e) {
      setMessage(String(e));
    }
  };

  return (
    <div className="receiver">
      <fieldset>
        <legend>{t("receiverStatus")}</legend>
        {info && !info.status ? (
          <p className="receiver-state error">
            {t("receiverUnavailable")} {info.initError}
          </p>
        ) : (
          <p className={`receiver-state ${status?.kind ?? "off"}`}>
            {status?.kind === "connected" &&
              t("receiverConnected", { name: status.device.product ?? "HID", id: vidPid(status.device), protocol: status.protocol })}
            {status?.kind === "searching" && t("receiverSearching", { id: vidPid(status.selector) })}
            {status?.kind === "error" && status.message}
            {(!status || status.kind === "off") && t("receiverOff")}
          </p>
        )}
        {status?.kind === "connected" && !status.hasLights && <p className="muted">{t("receiverNoLights")}</p>}
        {status && status.kind !== "off" && (
          <button onClick={() => run(api.receiverSelect(null))}>{t("forgetReceiver")}</button>
        )}
        {info?.platform === "linux" && udev && (
          <details>
            <summary>{t("linuxAccess")}</summary>
            <pre className="code">{udev}</pre>
            <button onClick={() => navigator.clipboard.writeText(udev)}>{t("copy")}</button>
          </details>
        )}
      </fieldset>

      <fieldset>
        <legend>{t("devices")}</legend>
        <div className="row">
          <button onClick={refreshDevices}>{t("refresh")}</button>
          <span className="muted">{t("devicesHint")}</span>
        </div>
        <table className="grid">
          <thead>
            <tr>
              <th>VID:PID</th>
              <th>{t("colProduct")}</th>
              <th>{t("colManufacturer")}</th>
              <th>{t("colUsage")}</th>
              <th>{t("colInterface")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {devices.map((d) => (
              <tr key={d.path} className={connected?.path === d.path ? "selected" : isKeyboardOrMouse(d) ? "disabled" : ""}>
                <td className="mono">{vidPid(d)}</td>
                <td>{d.product ?? "—"}</td>
                <td>{d.manufacturer ?? "—"}</td>
                <td className="mono">
                  {hex4(d.usagePage)}/{hex4(d.usage)} {isKeyboardOrMouse(d) && <span className="muted">({t("keyboardDevice")})</span>}
                </td>
                <td>{d.interfaceNumber}</td>
                <td>
                  {connected?.path === d.path ? (
                    t("inUse")
                  ) : (
                    <button onClick={() => run(api.receiverSelect(d))}>{t("useReceiver")}</button>
                  )}
                </td>
              </tr>
            ))}
            {devices.length === 0 && (
              <tr>
                <td colSpan={6} className="muted">
                  {t("noDevices")}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </fieldset>

      <fieldset>
        <legend>{t("learnTitle")}</legend>
        <p className="muted">{t("learnHint", { n: info?.learnedButtons ?? 0 })}</p>
        {learn?.active ? (
          <div className="row learn-active">
            <span>{t("learnPress", { button: buttonLabel(learn.button ?? "a"), buzzer: learn.buzzer ?? 0, n: learn.samples })}</span>
            <button disabled={learn.samples < 2} onClick={finishLearn}>
              {t("learnSave")}
            </button>
            <button onClick={() => run(api.receiverLearnCancel())}>{t("learnCancel")}</button>
          </div>
        ) : (
          <div className="row">
            <label className="inline">
              {t("learnBuzzer")}
              <input
                type="number"
                min={1}
                className="narrow"
                value={buzzer}
                onChange={(e) => setBuzzer(Math.max(1, Math.floor(Number(e.target.value) || 1)))}
              />
            </label>
            <label className="inline">
              {t("learnButton")}
              <select value={button} onChange={(e) => setButton(e.target.value as Button)}>
                {BUTTONS.map((b) => (
                  <option key={b} value={b}>
                    {buttonLabel(b)}
                  </option>
                ))}
              </select>
            </label>
            <button
              disabled={!connected}
              onClick={() => {
                setMessage(null);
                run(api.receiverLearnStart(buzzer, button));
              }}
            >
              {t("learnStart")}
            </button>
            <span className="spacer" />
            <button
              disabled={!info?.learnedButtons}
              onClick={() => {
                if (confirm(t("learnForgetConfirm"))) run(api.receiverForgetLearned().then(reload));
              }}
            >
              {t("learnForget")}
            </button>
          </div>
        )}
        {message && <p className="learn-message">{message}</p>}
      </fieldset>

      <fieldset className="live">
        <legend>{t("liveInput")}</legend>
        <div className="row">
          <span className="muted">{t("liveHint")}</span>
          <span className="spacer" />
          <button onClick={exportLog}>{t("exportLog")}</button>
        </div>
        <div className="live-logs">
          <div>
            <h4>{t("presses")}</h4>
            <ul className="log">
              {presses.map((p, i) => (
                <li key={i} className={p.used ? "used" : ""}>
                  {new Date(p.at).toLocaleTimeString()} · {pressLabel(p.press)}
                  {p.used && ` · ${t("pressUsed")}`}
                </li>
              ))}
            </ul>
          </div>
          <div>
            <h4>{t("rawReports")}</h4>
            <ul className="log mono">
              {raw.map((r, i) => (
                <li key={i}>
                  {hexBytes(r.bytes)}
                  {r.decoded.length > 0 && <span className="decoded"> → {r.decoded.map(pressLabel).join(", ")}</span>}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </fieldset>
    </div>
  );
}
