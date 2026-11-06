import { useState } from "react";
import { api } from "../api";
import { useApp, useReport } from "../app-context";
import { useGameView } from "../hooks";
import {
  setVoiceModel,
  useVoiceModel,
  useVoiceSample,
  VOICES,
  voiceName,
} from "../audio/read-aloud";
import { QUIZ_LANGUAGES } from "../bindings/Constants";
import { confirmAction } from "../confirm";
import { diskSize } from "../format";
import { PlayIcon, StopIcon } from "../icons";
import { LANGUAGE_LABEL } from "../quiz";
import type { VoiceDownload } from "../types";

/**
 * The voices questions are read in (spec SN-10): a sample of each voice in
 * each language, which plays before anything is downloaded, and the one
 * download that brings them all.
 */
export function VoicesSettings() {
  const { t } = useApp();
  const report = useReport();
  const model = useVoiceModel();
  const sample = useVoiceSample();
  // Null while unpacking, undefined when nothing is being downloaded.
  const [progress, setProgress] = useState<VoiceDownload | null | undefined>(
    undefined,
  );
  // The download writes much to the disk the show is saved to (design §1.1).
  const showRunning = useGameView() !== null;
  const busy = progress !== undefined || showRunning;

  const install = () => {
    setProgress(null);
    api
      .voiceInstall((done) =>
        setProgress(done.received < done.total ? done : null),
      )
      .then(setVoiceModel, report)
      .finally(() => setProgress(undefined));
  };
  const remove = () => {
    void confirmAction(t, t("voicesRemoveConfirm")).then((yes) => {
      if (!yes) return;
      setProgress(null);
      api
        .voiceRemove()
        .then(setVoiceModel, report)
        .finally(() => setProgress(undefined));
    });
  };
  const status = () => {
    if (progress === null) return t("voicesUnpacking");
    if (progress)
      return t("voicesDownloading", {
        percent: Math.floor((100 * progress.received) / progress.total),
      });
    return t(model?.installed ? "voicesInstalled" : "voicesNotInstalled");
  };

  return (
    <fieldset className="voices">
      <legend>{t("voices")}</legend>
      <p className="muted">
        {t("voicesHint", { size: diskSize(model?.bytes ?? 0) })}
      </p>
      <div className="row">
        <span role="status">{status()}</span>
        {model && !model.installed && (
          <button disabled={busy} onClick={install}>
            {t("voicesDownload")} ({diskSize(model.bytes)})
          </button>
        )}
        {model?.installed && (
          <button disabled={busy} onClick={remove}>
            {t("voicesRemove")}
          </button>
        )}
      </div>
      {showRunning && <p className="muted">{t("voicesDuringShow")}</p>}
      <p className="muted">{t("voicesLicence")}</p>
      {QUIZ_LANGUAGES.map((language) => (
        <section
          key={language}
          className="voice-group"
          aria-label={t(LANGUAGE_LABEL[language])}
        >
          <h3>{t(LANGUAGE_LABEL[language])}</h3>
          <ul>
            {VOICES.map((voice) => {
              const playing = sample.playing === `${language}-${voice}`;
              return (
                <li key={voice}>
                  <button
                    className="icon-button"
                    aria-label={`${t(playing ? "stopSample" : "playSample")}: ${voiceName(t, voice)}`}
                    title={t(playing ? "stopSample" : "playSample")}
                    onClick={() =>
                      playing ? sample.stop() : sample.play(language, voice)
                    }
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
