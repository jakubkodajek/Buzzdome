import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { setVoiceModel, useVoiceModel, useVoiceSample, VOICES, voiceName } from "../audio/read-aloud";
import { QUIZ_LANGUAGES } from "../bindings/Constants";
import { confirmAction } from "../confirm";
import { diskSize } from "../format";
import { errorNotice, isAppError } from "../messages";
import { PlayIcon, StopIcon } from "../icons";
import { LANGUAGE_LABEL } from "../quiz";
import { createStore, useStore } from "../store";
import type { VoiceDownload } from "../types";

// How far the download or removal got: null while unpacking or removing,
// undefined when nothing is under way. Kept outside the tab, so that it
// shows again when the quizmaster comes back to the tab meanwhile.
const progress = createStore<VoiceDownload | null | undefined>(undefined);

/** Runs `call` with the progress shown as `null` until it ends. */
function whileBusy<T>(call: () => Promise<T>): Promise<T> {
  progress.set(null);
  return call().finally(() => progress.set(undefined));
}

/** Whether the voices download stopped because a show began or the quizmaster cancelled it (`voice_install`). */
function downloadStopped(error: unknown): boolean {
  return isAppError(error) && (error.code === "voiceDownloadStopped" || error.code === "voiceDownloadCancelled");
}

/**
 * The voices questions are read in (spec SN-10): a sample of each voice in
 * each language, which plays before anything is downloaded, and the one
 * download that brings them all.
 */
export function VoicesSettings({ showRunning }: { showRunning: boolean }) {
  const { t, caution } = useApp();
  const run = useRun();
  const model = useVoiceModel();
  const sample = useVoiceSample();
  const done = useStore(progress);
  // The download writes much to the disk the show is saved to (design §1.1).
  const busy = done !== undefined || showRunning;

  // A show that began stopped the download: nothing failed (BF118).
  const install = () =>
    run(
      whileBusy(() => api.voiceInstall((got) => progress.set(got.received < got.total ? got : null))).then(setVoiceModel, (error: unknown) => {
        if (!downloadStopped(error)) throw error;
        caution(errorNotice(t, error));
      }),
    );
  const remove = () =>
    run(confirmAction(t, t("voicesRemoveConfirm"), t("voicesRemove")).then((yes) => (yes ? whileBusy(api.voiceRemove).then(setVoiceModel) : undefined)));
  const status = () => {
    if (done === null) return t("voicesUnpacking");
    if (done)
      return t("voicesDownloading", {
        percent: Math.floor((100 * done.received) / done.total),
        received: diskSize(done.received),
        total: diskSize(done.total),
      });
    return t(model?.installed ? "voicesInstalled" : "voicesNotInstalled");
  };

  return (
    <fieldset className="voices">
      <legend>{t("voices")}</legend>
      <p className="muted">{t("voicesHint", { size: diskSize(model?.bytes ?? 0) })}</p>
      <div className="row">
        <span role="status">{status()}</span>
        {model && !model.installed && (
          <button disabled={busy} onClick={install}>
            {t("voicesDownload")} ({diskSize(model.bytes)})
          </button>
        )}
        {done && (
          <button onClick={() => run(api.voiceInstallCancel())}>
            {t("voicesCancel")}
          </button>
        )}
        {model?.installed && (
          <button disabled={busy} onClick={remove}>
            {t("voicesRemove")}
          </button>
        )}
      </div>
      {showRunning && <p className="muted">{t("voicesDuringShow")}</p>}
      {QUIZ_LANGUAGES.map((language) => (
        <section key={language} className="voice-group" aria-label={t(LANGUAGE_LABEL[language])}>
          <h3>{t(LANGUAGE_LABEL[language])}</h3>
          <ul>
            {VOICES.map((voice) => {
              const playing = sample.plays(language, voice);
              return (
                <li key={voice}>
                  <button
                    className="icon-button"
                    aria-label={`${t(playing ? "stopSample" : "playSample")}: ${voiceName(t, voice)}`}
                    title={t(playing ? "stopSample" : "playSample")}
                    onClick={() => (playing ? sample.stop() : sample.play(language, voice))}
                  >
                    {playing ? <StopIcon /> : <PlayIcon />}
                  </button>
                  <span>{voiceName(t, voice)}</span>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </fieldset>
  );
}
