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
import { CrossIcon, PictureIcon, SoundIcon, SparkleIcon, TextIcon, VideoIcon } from "../icons";
import { diskSize } from "../format";
import { errorNotice, errorText, isAppError, type Notice } from "../messages";
import { NoticeBox } from "../notice";
import type { Translate } from "../i18n";
import { createStore, useStore } from "../store";
import type { Attachment, ListenerModel, LocalModelInfo, LocalModelProgress, QuizLanguage, Slide, Source, VoiceDownload } from "../types";
import { MAX_DROPPED_MB } from "../bindings/Constants";
import { keyCombo } from "../keyboard";
import { tooltip } from "./actions";
import { addFiles, aiFileKind, attachmentFor, hasSound, soundOf, thumbnailOf, type AiFile } from "./ai-files";

/** The keys that ask, as in a chat (Cmd on a Mac, `keyCombo`). */
const GENERATE_KEYS = ["Ctrl+Enter"];

/** Where a request is: the files prepared, the speech in one written down, then as the backend tells (`LocalModelProgress`). */
type Job = { step: "preparing" } | { step: "listening"; name: string } | { step: "waiting" } | LocalModelProgress;

interface PanelState {
  files: AiFile[];
  prompt: string;
  /** Whether the request may be looked up on the internet. */
  internet: boolean;
  job: Job | null;
  /** Why the last request failed, until the next one. */
  failed: Notice | null;
  /** The web pages the last questions were written from. */
  sources: Source[];
  /** The speech recognition model's download under way: how far, or null before the first report. */
  listenerDownload: VoiceDownload | null | undefined;
}

const panelState = createStore<PanelState>({ files: [], prompt: "", internet: true, job: null, failed: null, sources: [], listenerDownload: undefined });
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
  const { files, prompt, internet, job, failed, sources, listenerDownload } = useStore(panelState);
  const [info, setInfo] = useState<LocalModelInfo | null>(null);
  const [listener, setListener] = useState<ListenerModel | null>(null);
  const [dropping, setDropping] = useState(false);
  const [picked, setPicked] = useState<HTMLInputElement | null>(null);
  useEffect(() => {
    // Read again each time the panel opens: the model may have been chosen meanwhile.
    api.aiLocalModel().then(setInfo, (error: unknown) => change({ failed: errorNotice(t, error) }));
    api.listenerModel().then(setListener, () => setListener(null));
  }, [t]);
  const downloading = listenerDownload !== undefined;
  const installListener = async () => {
    change({ listenerDownload: null, failed: null });
    try {
      setListener(await api.listenerInstall((got) => change({ listenerDownload: got })));
    } catch (error) {
      if (!(isAppError(error) && error.code === "listenerDownloadCancelled")) change({ failed: errorNotice(t, error) });
    } finally {
      change({ listenerDownload: undefined });
    }
  };

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
    if (!listener?.installed && panelState.get().files.some((file) => file.kind === "recording")) {
      caution(t("aiPanelListenerRequired"));
      return;
    }
    change({ job: { step: "preparing" }, failed: null, sources: [] });
    try {
      const attachments: Attachment[] = [];
      let textLeft = info.maxTextChars;
      for (const listed of panelState.get().files) {
        let file = listed;
        // What is said in it, written down once per language while it stays in the panel.
        if (listener?.installed && hasSound(file.kind) && file.heard?.language !== language) {
          change({ job: { step: "listening", name: file.file.name } });
          const sound = await soundOf(file.file, listener.rate, listener.maxSeconds);
          if (!current()) return;
          if (!sound && file.kind === "recording") caution(t("aiPanelNoSound", { name: file.file.name }));
          const words = sound ? await api.editorTranscribe(file.file.name, language, sound) : "";
          if (!current()) return;
          file = { ...file, heard: { language, words } };
          const heard = file.heard;
          change({ files: panelState.get().files.map((f) => (f.id === file.id ? { ...f, heard } : f)), job: { step: "preparing" } });
        }
        let reference: string | null = null;
        if (file.kind === "picture" || file.kind === "video") {
          // The slides show the draft's own copy; copied once per draft.
          reference = file.copied?.draft === draft ? file.copied.reference : await api.editorAddDroppedMedia(draft, file.file);
          const copied = { draft, reference };
          change({ files: panelState.get().files.map((f) => (f.id === file.id ? { ...f, copied } : f)) });
        }
        let attachment: Attachment;
        try {
          attachment = await attachmentFor(file, info, reference, textLeft);
        } catch {
          if (!current()) return;
          // A file the window cannot decode is left out, and said so.
          caution(t("aiPanelFileFailed", { name: file.file.name }));
          continue;
        }
        const content = attachment.content;
        textLeft -= content.kind === "text" ? content.text.length : content.kind === "recording" || content.kind === "video" ? content.words.length : 0;
        attachments.push(attachment);
        if (!current()) return;
      }
      // Cancelled while the last file was prepared: nothing is asked.
      if (!current()) return;
      change({ job: { step: "waiting" } });
      const asked = await api.editorAskLocalModel(draft, { prompt, attachments, language, wholeQuiz: emptyQuiz, internet }, (progress) => {
        if (current()) change({ job: progress });
      });
      if (!current()) return;
      const inserted = onInsert(draft, asked.slides);
      change({ sources: asked.sources });
      if (inserted === null) caution(t("aiPanelOtherQuiz"));
      else notify(t("aiPanelInserted", { n: inserted }));
      if (asked.webFailed) caution(t(asked.sources.length > 0 ? "aiPanelWebPartly" : "aiPanelWebFailed"));
    } catch (error) {
      if (isAppError(error) && error.code === "localModelStoppedForShow") caution(errorText(t, error));
      else if (current() && !(isAppError(error) && error.code === "localModelCancelled")) change({ failed: errorNotice(t, error) });
    } finally {
      if (current()) change({ job: null });
    }
  };
  const cancel = () => {
    // The panel is free again at once; files being prepared stop at the
    // next, and a request under way ends in the backend too.
    const step = job?.step;
    latestRun++;
    change({ job: null });
    if (step === "listening") void api.editorTranscribeCancel();
    else if (step !== "preparing") void api.editorAskLocalModelCancel();
  };

  const chosen = info?.chosen ?? null;
  const promptMax = info?.maxPromptChars;
  const status = job ? jobStatus(t, job) : null;
  /** Neither a file nor a request: nothing to make questions from. */
  const nothingAsked = files.length === 0 && prompt.trim() === "";
  // What failed stays in view by the buttons: the panel's body scrolls, and its end is out of sight on a laptop.
  const failure = failed && <NoticeBox notice={failed} />;

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
                accept="image/*,video/mp4,video/webm,audio/*,.txt,.md,.csv,text/plain,text/markdown,text/csv"
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
            {listener && !listener.installed && files.some((file) => hasSound(file.kind)) && (
              <div className="ai-listener">
                <p>{t("aiPanelListenerNeeded", { size: diskSize(listener.bytes) })}</p>
                {downloading ? (
                  <div className="ai-listener-progress">
                    <span role="status">{t("aiPanelListenerDownloading", { done: diskSize(listenerDownload?.received ?? 0), total: diskSize(listener.bytes) })}</span>
                    <button type="button" className="browse" onClick={() => void api.listenerInstallCancel()}>
                      {t("aiPanelListenerCancel")}
                    </button>
                  </div>
                ) : (
                  <button type="button" className="browse" disabled={job !== null} onClick={() => void installListener()}>
                    {t("aiPanelListenerDownload")}
                  </button>
                )}
              </div>
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
                  if (keyCombo(e) === GENERATE_KEYS[0] && !job && chosen && !nothingAsked) {
                    e.preventDefault();
                    void generate();
                  }
                }}
              />
            </label>
            <label className="ai-internet">
              <input type="checkbox" checked={internet} disabled={job !== null} onChange={(e) => change({ internet: e.target.checked })} />
              <span>
                {t("aiPanelInternet")}
                <small>{t("aiPanelInternetHint")}</small>
              </span>
            </label>
            {!chosen && failure}
            {sources.length > 0 && !job && (
              <div className="ai-sources">
                <span>{t("aiPanelSources")}</span>
                <ul>
                  {sources.map((source) => (
                    <li key={source.url} title={source.url}>
                      {source.title} <small>{hostOf(source.url)}</small>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}
      </div>

      {chosen && (
        <footer className="ai-panel-footer">
          {failure}
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
                <button type="button" className="ai-generate" disabled={nothingAsked} title={tooltip({ label: t("aiPanelGenerate"), keys: GENERATE_KEYS })} onClick={() => void generate()}>
                  <SparkleIcon /> {t("aiPanelGenerate")}
                </button>
              </>
            )}
          </div>
          <p className="ai-panel-privacy">{t(internet ? "aiPanelPrivacyInternet" : "aiPanelPrivacy", { url: chosen.url })}</p>
          <button type="button" className="ai-change-model" disabled={job !== null} onClick={onChooseModel}>
            {t("aiPanelChangeModel")}
          </button>
        </footer>
      )}
    </aside>
  );
}

/** What the panel says while `job` is under way. */
function jobStatus(t: Translate, job: Job): string {
  switch (job.step) {
    case "preparing":
      return t("aiPanelPreparing");
    case "listening":
      return t("aiPanelListening", { name: job.name });
    case "waiting":
      return t("aiPanelWaiting");
    case "planning":
      return t("aiPanelPlanning");
    case "searching":
      return t("aiPanelSearching", { query: job.query });
    case "reading":
      return t("aiPanelReading", { page: hostOf(job.url) + new URL(job.url, "https://x").pathname.replace(/\/$/, "") });
    case "thinking":
      return t("aiPanelThinking", { n: job.characters });
    case "writing":
      return t("aiPanelWriting", { n: job.questions });
  }
}

/** The site of the web address `url`, without `www.`. */
function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return url;
  }
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
        {preview ? <img src={preview} alt="" /> : file.kind === "video" ? <VideoIcon /> : file.kind === "recording" ? <SoundIcon /> : file.kind === "text" ? <TextIcon /> : <PictureIcon />}
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
