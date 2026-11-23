// Studio's AI assistant (docs/ai-assistants.md): a panel beside the slides
// where the quizmaster adds pictures, videos and text files, says what
// questions they want, and a model on this computer writes them. The
// questions come back as slides that the editor inserts after the selected
// one as one change, as a spreadsheet's do.
//
// What the panel holds (files, request, a request under way) is kept while
// it is closed, so closing it to look at a slide loses nothing.

import { useEffect, useState, type DragEvent } from "react";
import { api } from "../api";
import { useApp } from "../app-context";
import { carriesFiles } from "../director/media-parts";
import { CrossIcon, PictureIcon, SparkleIcon, TextIcon, VideoIcon } from "../icons";
import { errorNotice, isAppError, type Notice } from "../messages";
import { NoticeBox } from "../notice";
import { createStore, useStore } from "../store";
import type { Attachment, LocalModelInfo, QuizLanguage, Slide } from "../types";
import { MAX_DROPPED_MB } from "../bindings/Constants";
import { addFiles, aiFileKind, attachmentFor, thumbnailOf, type AiFile } from "./ai-files";

/** Where a request is. */
type Job = { phase: "preparing" } | { phase: "waiting" } | { phase: "writing"; questions: number };

interface PanelState {
  files: AiFile[];
  prompt: string;
  job: Job | null;
  /** Why the last request failed, until the next one. */
  failed: Notice | null;
}

const panelState = createStore<PanelState>({ files: [], prompt: "", job: null, failed: null });
const change = (next: Partial<PanelState>) => panelState.set({ ...panelState.get(), ...next });

/**
 * The latest request: one that is cancelled or followed by another stops
 * changing the panel, even while it still waits on a file or the model.
 */
let latestRun = 0;

export interface AiPanelProps {
  /** The open draft (`EditorDraft.id`). */
  draftId: string;
  language: QuizLanguage;
  /** Whether the quiz has no slides yet: the questions then make a whole quiz, ending with the results. */
  emptyQuiz: boolean;
  /** Inserts the slides the model wrote into the draft `draft`; the number inserted, or null when another draft is open now. */
  onInsert: (draft: string, slides: Slide[]) => number | null;
  /** Opens File › AI assistants, where the model is chosen. */
  onChooseModel: () => void;
  onClose: () => void;
}

export function AiPanel({ draftId, language, emptyQuiz, onInsert, onChooseModel, onClose }: AiPanelProps) {
  const { t, notify, caution } = useApp();
  const { files, prompt, job, failed } = useStore(panelState);
  const [info, setInfo] = useState<LocalModelInfo | null>(null);
  const [dropping, setDropping] = useState(false);
  const [picked, setPicked] = useState<HTMLInputElement | null>(null);
  useEffect(() => {
    // Read again each time the panel opens: the model may have been chosen meanwhile.
    api.aiLocalModel().then(setInfo, (error: unknown) => change({ failed: errorNotice(t, error) }));
  }, [t]);

  const add = (given: File[]) => {
    if (!info || given.length === 0) return;
    // A picture or video is copied into the quiz whole, so one too large for that is not taken (a text file is read only in part).
    const tooLarge = given.find((file) => aiFileKind(file.name) !== "text" && file.size > MAX_DROPPED_MB * 2 ** 20);
    if (tooLarge) caution(t("mediaDropTooLarge", { name: tooLarge.name, mb: MAX_DROPPED_MB }));
    const dropped = given.filter((file) => aiFileKind(file.name) === "text" || file.size <= MAX_DROPPED_MB * 2 ** 20);
    const added = addFiles(panelState.get().files, dropped, info.maxAttachments);
    change({ files: added.files });
    if (tooLarge) return;
    if (added.unsupported.length > 0) caution(t("aiPanelUnsupported", { names: added.unsupported.join(", ") }));
    else if (added.tooMany) caution(t("aiPanelTooMany", { max: info.maxAttachments }));
  };
  const onDragOver = (e: DragEvent) => {
    if (!carriesFiles(e) || job) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    setDropping(true);
  };
  const onDrop = (e: DragEvent) => {
    if (!carriesFiles(e) || job) return;
    e.preventDefault();
    setDropping(false);
    add([...e.dataTransfer.files]);
  };

  const generate = async () => {
    if (!info?.chosen || panelState.get().job) return;
    const draft = draftId;
    const run = ++latestRun;
    const current = () => run === latestRun;
    change({ job: { phase: "preparing" }, failed: null });
    try {
      const attachments: Attachment[] = [];
      let textLeft = info.maxTextChars;
      for (const file of panelState.get().files) {
        let reference: string | null = null;
        if (file.kind !== "text") {
          // The slides show the draft's own copy; copied once per draft.
          reference = file.copied?.draft === draft ? file.copied.reference : await api.editorAddDroppedMedia(draft, file.file);
          const copied = { draft, reference };
          change({ files: panelState.get().files.map((f) => (f.id === file.id ? { ...f, copied } : f)) });
        }
        const attachment = await attachmentFor(file, info, reference, textLeft);
        if (attachment.content.kind === "text") textLeft -= attachment.content.text.length;
        attachments.push(attachment);
        if (!current()) return;
      }
      change({ job: { phase: "waiting" } });
      const slides = await api.editorAskLocalModel(draft, { prompt, attachments, language, wholeQuiz: emptyQuiz }, (progress) => {
        if (current()) change({ job: { phase: "writing", questions: progress.questions } });
      });
      if (!current()) return;
      const inserted = onInsert(draft, slides);
      if (inserted === null) caution(t("aiPanelOtherQuiz"));
      else notify(t("aiPanelInserted", { n: inserted }));
    } catch (error) {
      if (current() && !(isAppError(error) && error.code === "localModelCancelled")) change({ failed: errorNotice(t, error) });
    } finally {
      if (current()) change({ job: null });
    }
  };
  const cancel = () => {
    // The panel is free again at once; files being prepared stop at the
    // next, and a request under way ends in the backend too.
    const waited = job?.phase !== "preparing";
    latestRun++;
    change({ job: null });
    if (waited) void api.editorAskLocalModelCancel();
  };

  const chosen = info?.chosen ?? null;
  const promptMax = info?.maxPromptChars;
  const status =
    job?.phase === "preparing" ? t("aiPanelPreparing") : job?.phase === "waiting" ? t("aiPanelWaiting") : job ? t("aiPanelWriting", { n: job.questions }) : null;

  return (
    <aside className="ai-panel" aria-label={t("aiPanel")} onDragOver={onDragOver} onDragLeave={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && setDropping(false)} onDrop={onDrop}>
      <header className="props-header">
        <span className="props-icon">
          <SparkleIcon />
        </span>
        <div className="props-heading">
          <strong className="props-title">{t("aiPanel")}</strong>
          {chosen && (
            <small title={chosen.url}>
              {t("aiPanelModel", { model: chosen.model })}
            </small>
          )}
        </div>
        <button type="button" className="ai-panel-close" title={t("aiPanelClose")} aria-label={t("aiPanelClose")} onClick={onClose}>
          <CrossIcon />
        </button>
      </header>

      <div className="ai-panel-body">
        {info && !chosen ? (
          <div className="ai-panel-setup">
            <p>{t("aiPanelNoModel")}</p>
            <button type="button" className="browse" onClick={onChooseModel}>
              {t("aiPanelChooseModel")}
            </button>
          </div>
        ) : (
          <>
            <p className="ai-panel-hint">{t("aiPanelHint")}</p>
            <div className={dropping ? "ai-drop dropping" : "ai-drop"}>
              <span>{t("aiPanelDrop")}</span>
              <button type="button" className="browse" disabled={job !== null || !info} onClick={() => picked?.click()}>
                {t("aiPanelAddFiles")}
              </button>
              <input
                ref={setPicked}
                type="file"
                multiple
                hidden
                accept="image/*,video/mp4,video/webm,.txt,.md,.csv,text/plain,text/markdown,text/csv"
                onChange={(e) => {
                  const chosenFiles = [...(e.target.files ?? [])];
                  e.target.value = "";
                  add(chosenFiles);
                }}
              />
            </div>
            {files.length === 0 ? (
              <p className="ai-panel-empty">{t("aiPanelEmpty")}</p>
            ) : (
              <ul className="ai-files" aria-label={t("aiPanelFiles")}>
                {files.map((file) => (
                  <FileRow key={file.id} file={file} disabled={job !== null} label={t("aiPanelRemoveFile", { name: file.file.name })} onRemove={() => change({ files: panelState.get().files.filter((f) => f.id !== file.id) })} />
                ))}
              </ul>
            )}
            <label className="ai-prompt">
              <span>{t("aiPanelPrompt")}</span>
              <textarea
                value={prompt}
                rows={5}
                maxLength={promptMax}
                placeholder={t("aiPanelPromptPlaceholder")}
                disabled={job !== null}
                onChange={(e) => change({ prompt: e.target.value })}
                onKeyDown={(e) => {
                  // Ctrl+Enter asks, as in a chat.
                  if (e.key === "Enter" && (e.ctrlKey || e.metaKey) && !job && chosen) {
                    e.preventDefault();
                    void generate();
                  }
                }}
              />
            </label>
            {failed && <NoticeBox notice={failed} />}
          </>
        )}
      </div>

      {chosen && (
        <footer className="ai-panel-footer">
          {status && (
            <p className="ai-panel-status" role="status">
              <span className="ai-spinner" aria-hidden="true" />
              {status}
            </p>
          )}
          <div className="ai-panel-buttons">
            {job ? (
              <button type="button" className="browse" onClick={cancel}>
                {t("aiPanelCancel")}
              </button>
            ) : (
              <>
                <button type="button" className="browse" disabled={files.length === 0 && prompt === ""} onClick={() => change({ files: [], prompt: "", failed: null })}>
                  {t("aiPanelClear")}
                </button>
                <button type="button" className="ai-generate" disabled={files.length === 0 && prompt.trim() === ""} title="Ctrl+Enter" onClick={() => void generate()}>
                  <SparkleIcon /> {t("aiPanelGenerate")}
                </button>
              </>
            )}
          </div>
          <p className="ai-panel-privacy">{t("aiPanelPrivacy", { url: chosen.url })}</p>
          <button type="button" className="ai-change-model" disabled={job !== null} onClick={onChooseModel}>
            {t("aiPanelChangeModel")}
          </button>
        </footer>
      )}
    </aside>
  );
}

/** A file in the list: a small picture or the kind's icon, its name, and a button that takes it out. */
function FileRow({ file, disabled, label, onRemove }: { file: AiFile; disabled: boolean; label: string; onRemove: () => void }) {
  const [preview, setPreview] = useState<string | null>(null);
  useEffect(() => {
    if (file.kind !== "picture") return;
    let current = true;
    void thumbnailOf(file.file).then((url) => current && setPreview(url));
    return () => {
      current = false;
    };
  }, [file.file, file.kind]);
  return (
    <li className="ai-file">
      <span className="ai-file-thumb" aria-hidden="true">
        {preview ? <img src={preview} alt="" /> : file.kind === "video" ? <VideoIcon /> : file.kind === "text" ? <TextIcon /> : <PictureIcon />}
      </span>
      <span className="ai-file-name" title={file.file.name}>
        {file.file.name}
      </span>
      <button type="button" title={label} aria-label={label} disabled={disabled} onClick={onRemove}>
        <CrossIcon />
      </button>
    </li>
  );
}
