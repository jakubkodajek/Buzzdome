import { save } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { BUTTONS, buttonLabel } from "../answers";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { useBackendEvent, useLearnProgress } from "../hooks";
import { errorText, protocolText, receiverProblemText, settingsLostText } from "../messages";
import type { Button, DeviceInfo, Press, PressEvent, RawReport, ReceiverInfo, ReceiverStatus } from "../types";

const LOG_LEN = 40;

const hex4 = (n: number) => n.toString(16).padStart(4, "0");
const vidPid = (d: { vendorId: number; productId: number }) => `${hex4(d.vendorId)}:${hex4(d.productId)}`;
const hexBytes = (bytes: number[]) => bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ");
const pressLabel = (p: Press) => `${p.buzzer} ${buttonLabel(p.button)}`;
const isKeyboardOrMouse = (d: DeviceInfo) => d.usagePage === 1 && (d.usage === 2 || d.usage === 6);

/** Receiver setup and diagnostics: pick the USB receiver, watch its raw data, teach buttons. */
export function ReceiverTab() {
  const { t } = useApp();
  const run = useRun();
  const [info, setInfo] = useState<ReceiverInfo | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [raw, setRaw] = useState<RawReport[]>([]);
  const [presses, setPresses] = useState<(PressEvent & { at: number })[]>([]);
  const [buzzer, setBuzzer] = useState(1);
  const [button, setButton] = useState<Button>("a");
  const [message, setMessage] = useState<string | null>(null);
  const [udev, setUdev] = useState("");

  const reload = useCallback(() => {
    run(
      api.receiverInfo().then((i) => {
        setInfo(i);
        setStatus(i.status);
        setRaw([...i.recent].reverse().slice(0, LOG_LEN));
      }),
    );
  }, [run]);

  const refreshDevices = useCallback(() => run(api.receiverDevices().then(setDevices)), [run]);

  useEffect(() => {
    reload();
    refreshDevices();
  }, [reload, refreshDevices]);

  useBackendEvent("receiver-status", (next) => {
    setStatus(next);
    reload();
  });
  useBackendEvent("receiver-raw", (report) => setRaw((list) => [report, ...list].slice(0, LOG_LEN)));
  useBackendEvent("receiver-press", (event) => setPresses((list) => [{ ...event, at: Date.now() }, ...list].slice(0, LOG_LEN)));
  const learn = useLearnProgress();
  // Learning stops with the tab, so presses never stay muted in a show.
  useEffect(() => () => void api.receiverLearnCancel().catch((e) => console.error(e)), []);

  useEffect(() => {
    if (info?.platform === "linux") api.receiverUdevRule().then(setUdev).catch(() => setUdev(""));
  }, [info?.platform, status]);

  const connected = status?.kind === "connected" ? status.device : null;

  const exportLog = async () => {
    const path = await save({ defaultPath: "receiver-log.txt", filters: [{ name: "Text", extensions: ["txt"] }] });
    if (path) run(api.receiverExportLog(path));
  };

  const finishLearn = async () => {
    try {
      const learned = await api.receiverLearnFinish();
      setMessage(t("learned", { buzzer: learned.buzzer, button: buttonLabel(learned.button) }));
      // Move on to the next button, then the next buzzer.
      const i = BUTTONS.indexOf(button);
      if (i < BUTTONS.length - 1) setButton(BUTTONS[i + 1]);
      else {
        setButton("a");
        setBuzzer(buzzer + 1);
      }
      reload();
    } catch (e) {
      setMessage(errorText(t, e));
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
              t("receiverConnected", { name: status.device.product ?? "HID", id: vidPid(status.device), protocol: protocolText(t, status.protocol) })}
            {status?.kind === "searching" && t("receiverSearching", { id: vidPid(status.selector) })}
            {status?.kind === "error" && receiverProblemText(t, status.problem, status.detail, info?.platform ?? "")}
            {(!status || status.kind === "off") && t("receiverOff")}
          </p>
        )}
        {status?.kind === "connected" && !status.hasLights && <p className="muted">{t("receiverNoLights")}</p>}
        {info?.settingsError && (
          <p className="receiver-state error">
            {t("receiverSettingsLost")} {settingsLostText(t, info.settingsError)}
          </p>
        )}
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
            <button disabled={!learn.canSave} onClick={finishLearn}>
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
