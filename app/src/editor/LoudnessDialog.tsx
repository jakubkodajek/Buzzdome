import { useEffect, useMemo, useState } from "react";
import { useApp } from "../app-context";
import { decodeAudioFile } from "../audio/decode";
import { LOUDNESS_TARGETS, measureLoudness, normalization, type Loudness, type LoudnessTarget } from "../audio/loudness";
import { decibels } from "../format";
import { mediaFileName, mediaUrl } from "../media";
import type { Quiz } from "../types";
import { Dialog, Select } from "./controls";
import { gainChange, LOUDNESS_STATUS_KEYS, LOUDNESS_TARGET_KEYS, readLoudnessTarget, writeLoudnessTarget } from "./normalize";

interface LoudnessDialogProps {
  quiz: Quiz;
  /** Folder the quiz's media references resolve against. */
  mediaRoot: string;
  /** Sets the loudness gain of the sounds on these slides (by id), as one undo step. */
  onApply: (gains: Map<string, number>) => void;
  onClose: () => void;
}

/** A measured sound: its loudness, or that its file could not be read. */
type Measured = Loudness | "failed";

/**
 * Measures every sound of the quiz and evens them out to one loudness
 * (SN-9): a row per sound with its loudness, peak, the change it needs and
 * why, ticked when it changes. Files are not changed; Apply sets each ticked
 * sound's loudness gain. The chosen target is remembered on this computer.
 */
export function LoudnessDialog({ quiz, mediaRoot, onApply, onClose }: LoudnessDialogProps) {
  const { t } = useApp();
  const rows = useMemo(() => quiz.slides.flatMap((slide, i) => (slide.sound ? [{ id: slide.id, n: i + 1, sound: slide.sound }] : [])), [quiz]);
  const [target, setTarget] = useState<LoudnessTarget>(readLoudnessTarget);
  const [measured, setMeasured] = useState<ReadonlyMap<string, Measured>>(new Map());
  const [unticked, setUnticked] = useState<ReadonlySet<string>>(new Set());

  // One file at a time, each decoded once however many slides play it, so
  // only one file's samples are in memory.
  useEffect(() => {
    let alive = true;
    void (async () => {
      for (const file of new Set(rows.map((row) => row.sound.file))) {
        const results = new Map<string, Measured>();
        try {
          const samples = await decodeAudioFile(mediaUrl(mediaRoot, file));
          for (const row of rows.filter((r) => r.sound.file === file)) results.set(row.id, measureLoudness(samples, row.sound.part));
        } catch (error) {
          console.error("loudness:", error);
          for (const row of rows.filter((r) => r.sound.file === file)) results.set(row.id, "failed");
        }
        if (!alive) return;
        setMeasured((done) => new Map([...done, ...results]));
      }
    })();
    return () => {
      alive = false;
    };
  }, [rows, mediaRoot]);

  const results = rows.map((row) => {
    const loudness = measured.get(row.id);
    const change = loudness && loudness !== "failed" ? gainChange(row.sound.gainDb, normalization(loudness, target)) : null;
    /** The new gain; null when it stays. */
    const gainDb = change?.gainDb ?? null;
    return { ...row, loudness, change, gainDb, ticked: gainDb !== null && !unticked.has(row.id) };
  });
  const done = measured.size === rows.length;
  const apply = () => {
    writeLoudnessTarget(target);
    onApply(new Map(results.flatMap((row) => (row.ticked && row.gainDb !== null ? [[row.id, row.gainDb]] : []))));
    onClose();
  };

  return (
    <Dialog
      title={t("normalizeAll")}
      className="loudness-dialog"
      onClose={onClose}
      footer={
        <>
          <span className="loudness-progress">{done ? t("loudnessDone", { n: rows.length }) : t("loudnessMeasuring", { done: measured.size, n: rows.length })}</span>
          <span className="spacer" />
          <button type="button" className="primary" disabled={!done || !results.some((row) => row.ticked)} onClick={apply}>
            {t("apply")}
          </button>
          <button type="button" onClick={onClose}>
            {t("cancel")}
          </button>
        </>
      }
    >
      <label className="loudness-target">
        {t("loudnessTarget")}
        <Select label={t("loudnessTarget")} value={target} options={LOUDNESS_TARGETS.map((value) => [value, t(LOUDNESS_TARGET_KEYS[value])])} onChange={setTarget} />
      </label>
      {rows.length === 0 ? (
        <p className="muted">{t("loudnessNoSounds")}</p>
      ) : (
        <table className="loudness-table">
          <thead>
            <tr>
              <th />
              <th>{t("loudnessSlide")}</th>
              <th>{t("loudnessSound")}</th>
              <th>{t("loudnessLufs")}</th>
              <th>{t("loudnessPeak")}</th>
              <th>{t("loudnessChange")}</th>
              <th>{t("loudnessStatus")}</th>
            </tr>
          </thead>
          <tbody>
            {results.map((row) => (
              <tr key={row.id}>
                <td>
                  <input
                    type="checkbox"
                    aria-label={mediaFileName(row.sound.file)}
                    checked={row.ticked}
                    disabled={row.gainDb === null}
                    onChange={(e) =>
                      setUnticked((current) => {
                        const next = new Set(current);
                        if (e.target.checked) next.delete(row.id);
                        else next.add(row.id);
                        return next;
                      })
                    }
                  />
                </td>
                <td className="tabular">{row.n}</td>
                <td className="loudness-file" title={row.sound.file}>
                  {mediaFileName(row.sound.file)}
                </td>
                <td className="tabular">{row.loudness && row.loudness !== "failed" && row.loudness.lufs !== null ? decibels(row.loudness.lufs, false) : ""}</td>
                <td className="tabular">{row.loudness && row.loudness !== "failed" && Number.isFinite(row.loudness.peakDb) ? decibels(row.loudness.peakDb, false) : ""}</td>
                <td className="tabular">{row.change ? decibels((row.gainDb ?? row.sound.gainDb) - row.sound.gainDb) : ""}</td>
                <td>{row.loudness === undefined ? t("loudnessWaiting") : t(LOUDNESS_STATUS_KEYS[row.change?.status ?? "failed"])}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Dialog>
  );
}
