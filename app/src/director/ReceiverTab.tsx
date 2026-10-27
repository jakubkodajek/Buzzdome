import { useCallback, useEffect, useState } from "react";
import { BUTTONS, buttonLabel } from "../answers";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { useBackendEvent, useLearnProgress } from "../hooks";
import type { Translate } from "../i18n";
import { awaitsAnswer, errorText, hex4, receiverStatusText, REMOTE_ACTIONS, remoteActionText, remoteKeyText, settingsLostText, vidPid } from "../messages";
import type { Button, BuzzerSeen, DeviceInfo, Press, PressEvent, RawReport, ReceiverInfo, ReceiverStatus, RemoteAction, RemoteKey, RemoteKeyAction } from "../types";
import { confirmAction } from "../confirm";

const LOG_LEN = 40;

const hexBytes = (bytes: number[]) => bytes.map((b) => b.toString(16).padStart(2, "0")).join(" ");
/** A press as the logs show it, with the buzzer's own timer when it sends one. */
const pressLabel = (t: Translate, p: Press) =>
  `${p.buzzer} ${buttonLabel(p.button)}${p.timer === null ? "" : ` · ${t("pressTimer", { timer: p.timer })}`}`;
const isKeyboardOrMouse = (d: DeviceInfo) => d.usagePage === 1 && (d.usage === 2 || d.usage === 6);

/** The status line's look: blue while the receiver is waited for. */
const stateClass = (status: ReceiverStatus | null) => {
  if (status?.kind === "connected") return awaitsAnswer(status.protocol) ? "searching" : "connected";
  return status?.kind === "looking" ? "searching" : (status?.kind ?? "off");
};

/**
 * Receiver setup and diagnostics: pick the USB receiver, see which buzzers it hears, choose what the remote's keys
 * do, watch its raw data, teach buttons.
 */
export function ReceiverTab() {
  const { t } = useApp();
  const run = useRun();
  const [info, setInfo] = useState<ReceiverInfo | null>(null);
  const [status, setStatus] = useState<ReceiverStatus | null>(null);
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [raw, setRaw] = useState<RawReport[]>([]);
  const [presses, setPresses] = useState<(PressEvent & { at: number })[]>([]);
  const [buzzers, setBuzzers] = useState<BuzzerSeen[]>([]);
  const [latest, setLatest] = useState<number | null>(null);
  const [remote, setRemote] = useState<RemoteKeyAction[]>([]);
  const [pressedKey, setPressedKey] = useState<RemoteKey | null>(null);
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
        setBuzzers(i.buzzers);
        setRemote(i.remote);
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
    // A receiver found by itself shows in the list.
    refreshDevices();
  });
  useBackendEvent("receiver-buzzer", (seen) => {
    setBuzzers((list) => [...list.filter((b) => b.buzzer !== seen.buzzer), seen].sort((a, b) => a.buzzer - b.buzzer));
    setLatest(seen.buzzer);
  });
  useBackendEvent("receiver-raw", (report) => setRaw((list) => [report, ...list].slice(0, LOG_LEN)));
  useBackendEvent("receiver-remote", ({ key }) => setPressedKey(key));
  // eslint-disable-next-line no-restricted-properties -- the time of day a press came, as the log shows it, not the show clock
  useBackendEvent("receiver-press", (event) => setPresses((list) => [{ ...event, at: Date.now() }, ...list].slice(0, LOG_LEN)));
  const learn = useLearnProgress();
  // Learning stops with the tab, so presses never stay muted in a show.
  useEffect(() => () => void api.receiverLearnCancel().catch((e) => console.error(e)), []);

  useEffect(() => {
    if (info?.platform === "linux") api.receiverUdevRule().then(setUdev).catch(() => setUdev(""));
  }, [info?.platform, status]);

  const connected = status?.kind === "connected" ? status.device : null;
  // A receiver whose protocol the app knows has nothing to teach.
  const teaches = status?.kind !== "connected" || status.learnable;

  const setRemoteAction = (key: RemoteKey, action: RemoteAction) => run(api.receiverSetRemoteAction(key, action).then(reload));

  const exportLog = () => run(api.receiverExportLog({ filter: t("textFile"), fileName: "receiver-log" }));

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
          <p className={`receiver-state ${stateClass(status)}`}>{status && receiverStatusText(t, status, info?.platform ?? "")}</p>
        )}
        {status?.kind === "connected" && info && !info.chosen && <p className="muted">{t("receiverFoundAutomatically")}</p>}
        {status?.kind === "connected" && !status.hasLights && <p className="muted">{t("receiverNoLights")}</p>}
        {info?.settingsError && (
          <p className="receiver-state error">
            {t("receiverSettingsLost")} {settingsLostText(t, info.settingsError)}
          </p>
        )}
        {info?.chosen && (
          <button title={t("findReceiverAutomaticallyHint")} onClick={() => run(api.receiverSelect(null).then(reload))}>
            {t("findReceiverAutomatically")}
          </button>
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
        <legend>{t("buzzersSeen")}</legend>
        <p className="muted">{t("buzzersSeenHint")}</p>
        {buzzers.length > 0 ? (
          <ul className="buzzers-seen">
            {buzzers.map((b) => (
              <li key={b.buzzer} className={b.buzzer === latest ? "latest" : ""}>
                <strong>{b.buzzer}</strong>
                <span>{buttonLabel(b.button)}</span>
                <span className="muted">{new Date(b.atMs).toLocaleTimeString()}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">{t("noBuzzersSeen")}</p>
        )}
      </fieldset>

      <fieldset>
        <legend>{t("remoteKeys")}</legend>
        <p className="muted">{t("remoteKeysHint")}</p>
        <table className="grid remote-keys">
          <tbody>
            {remote.map(({ key, action }) => (
              <tr key={key} className={key === pressedKey ? "selected" : ""}>
                <th scope="row">{remoteKeyText(t, key)}</th>
                <td>
                  <select aria-label={remoteKeyText(t, key)} value={action} onChange={(e) => setRemoteAction(key, e.target.value as RemoteAction)}>
                    {REMOTE_ACTIONS.map((a) => (
                      <option key={a} value={a}>
                        {remoteActionText(t, a)}
                      </option>
                    ))}
                  </select>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="row">
          {pressedKey && <span>{t("remoteKeyPressed", { key: remoteKeyText(t, pressedKey) })}</span>}
          <span className="spacer" />
          <button onClick={() => run(api.receiverResetRemote().then(reload))}>{t("remoteKeysReset")}</button>
        </div>
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
                    <>
                      {t("inUse")}{" "}
                      <button title={t("reconnectReceiverHint")} onClick={() => run(api.receiverSelect(info?.chosen ? d : null))}>
                        {t("reconnectReceiver")}
                      </button>
                    </>
                  ) : (
                    <button onClick={() => run(api.receiverSelect(d).then(reload))}>{t("useReceiver")}</button>
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

      {teaches && (
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
                onClick={() => run(confirmAction(t, t("learnForgetConfirm")).then((yes) => (yes ? api.receiverForgetLearned().then(reload) : undefined)))}
              >
                {t("learnForget")}
              </button>
            </div>
          )}
          {message && <p className="learn-message">{message}</p>}
        </fieldset>
      )}

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
                  {new Date(p.at).toLocaleTimeString()} · {pressLabel(t, p.press)}
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
                  {r.decoded.length > 0 && <span className="decoded"> → {r.decoded.map((p) => pressLabel(t, p)).join(", ")}</span>}
                  {r.remote.length > 0 && (
                    <span className="decoded"> → {t("remoteInLog", { keys: r.remote.map((k) => remoteKeyText(t, k)).join(", ") })}</span>
                  )}
                  {r.dropped.length > 0 && (
                    <span className="muted">
                      {" "}
                      ✕ {r.dropped.map((p) => pressLabel(t, p)).join(", ")}: {t("droppedNotReady")}
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </fieldset>
    </div>
  );
}
