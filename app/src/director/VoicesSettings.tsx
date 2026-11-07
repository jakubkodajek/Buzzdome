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
import { cancellable, downloadReported, voicesStatus, type VoicesWork } from "./voices";

// The download or removal under way, null when none is. Kept outside the
// tab, so that it shows again when the quizmaster comes back to the tab
// meanwhile.
const work = createStore<VoicesWork | null>(null);

/** Runs `call` with `start` as the work under way until it ends. */
function busyWith<T>(start: VoicesWork, call: () => Promise<T>): Promise<T> {
  work.set(start);
  return call().finally(() => work.set(null));
}

/** Whether the voices download stopped because a show began or the quizmaster cancelled it (`voice_install`). */
function downloadStopped(error: unknown): boolean {
  return isAppError(error) && (error.code === "voiceDownloadStopped" || error.code === "voiceDownloadCancelled");
}

/**
 * The voices questions are read in (spec SN-10): a sample of each voice in
 * each language, which plays before anything is downloaded, and the one
 * download that brings them all. During a show the samples do not play:
 * they would be heard in the hall, and the window's one voice reads the
 * question.
 */
export function VoicesSettings({ showRunning }: { showRunning: boolean }) {
  const { t, caution } = useApp();
  const run = useRun();
  const model = useVoiceModel();
  const sample = useVoiceSample();
  const now = useStore(work);
  // The download writes much to the disk the show is saved to (design §1.1).
  const busy = now !== null || showRunning;

  // A show that began stopped the download: nothing failed (BF118).
  const install = () =>
    run(
      busyWith({ phase: "connecting" }, () => api.voiceInstall((got) => work.set(downloadReported(work.get(), got)))).then(setVoiceModel, (error: unknown) => {
        if (!downloadStopped(error)) throw error;
        caution(errorNotice(t, error));
      }),
    );
  const cancel = () => {
    work.set({ phase: "cancelling" });
    run(api.voiceInstallCancel());
  };
  const remove = () =>
    run(confirmAction(t, t("voicesRemoveConfirm"), t("voicesRemove")).then((yes) => (yes ? busyWith({ phase: "removing" }, api.voiceRemove).then(setVoiceModel) : undefined)));

  return (
    <fieldset className="voices">
      <legend>{t("voices")}</legend>
      <p className="muted">{t("voicesHint", { size: diskSize(model?.bytes ?? 0) })}</p>
      <div className="row">
        <span role="status">{voicesStatus(t, now, model)}</span>
        {model && !model.installed && (
          <button disabled={busy} onClick={install}>
            {t("voicesDownload")} ({diskSize(model.bytes)})
          </button>
        )}
        {cancellable(now) && (
          <button disabled={now?.phase === "cancelling"} onClick={cancel}>
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
                    disabled={showRunning && !playing}
                    aria-label={`${t(playing ? "stopSample" : "playSample")}: ${voiceName(t, voice)}`}
                    title={showRunning && !playing ? t("voicesDuringShow") : t(playing ? "stopSample" : "playSample")}
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
