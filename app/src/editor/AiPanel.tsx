// Studio's AI assistant (docs/ai-assistants.md): a chat beside the slides
// with a model on this computer. The quizmaster says what quiz they want,
// adds pictures, videos, sounds or text with the + button (or drops them on
// the panel), and the model proposes rounds and questions; once the
// proposal is right, Make questions has the model write them from the
// conversation. They come back as slides that the editor inserts after the
// selected one as one change, as a spreadsheet's do.
//
// What the panel holds (the conversation, files, the message being typed, a
// request under way) is kept while it is closed, so closing it to look at a
// slide loses nothing.

import { useEffect, useRef, useState, type DragEvent } from "react";
import { api } from "../api";
import { useApp } from "../app-context";
import { carriesFiles } from "../director/media-parts";
import { AddIcon, ArrowUpIcon, CheckIcon, CopyIcon, CrossIcon, EditIcon, GlobeIcon, PictureIcon, SoundIcon, SparkleIcon, StopIcon, TextIcon, VideoIcon } from "../icons";
import { diskSize } from "../format";
import { errorNotice, errorText, isAppError, type Notice } from "../messages";
import { NoticeBox } from "../notice";
import type { Translate } from "../i18n";
import { createStore, useStore } from "../store";
import type { Attachment, ListenerModel, LocalModelInfo, LocalModelProgress, QuestionRequest, QuizLanguage, Slide, VoiceDownload } from "../types";
import { MAX_DROPPED_MB } from "../bindings/Constants";
import { keyCombo } from "../keyboard";
import { tooltip } from "./actions";
import { addFiles, aiFileKind, attachmentFor, hasSound, soundOf, thumbnailOf, type AiFile } from "./ai-files";
import { canCreate, canSend, takeBack, entry, replyBlocks, turnsOf, unsent, type ChatEntry, type Run } from "./ai-chat";

/** The keys that make the questions (Cmd on a Mac, `keyCombo`); Enter alone sends the message, as in a chat. */
const CREATE_KEYS = ["Ctrl+Enter"];

/** Where a request is: the files prepared, the speech in one written down, then as the backend tells (`LocalModelProgress`). */
type Job = ({ step: "preparing" } | { step: "listening"; name: string } | { step: "waiting" } | LocalModelProgress) & { making: boolean };

interface PanelState {
  entries: ChatEntry[];
  /** The quizmaster's message the running request answers, taken back if it fails or is cancelled (`takeBack`). */
  pending: number | null;
  /** The message being typed. */
  message: string;
  files: AiFile[];
  /** Whether the model's turns and the questions may be looked up on the internet. */
  internet: boolean;
  /** What was found on the internet so far in the conversation (`ChatReply.notes`). */
  notes: string | null;
  job: Job | null;
  /** The model's turn being written, as it comes. */
  saying: string;
  /** Why the last request failed, until the next one. */
  failed: Notice | null;
  /** The speech recognition model's download under way: how far, or null before the first report. */
  listenerDownload: VoiceDownload | null | undefined;
}

const START: PanelState = { entries: [], pending: null, message: "", files: [], internet: true, notes: null, job: null, saying: "", failed: null, listenerDownload: undefined };
const panelState = createStore<PanelState>(START);
const change = (next: Partial<PanelState>) => panelState.set({ ...panelState.get(), ...next });

/**
 * The latest request: one that is cancelled or followed by another stops
 * changing the panel, even while it still waits on a file or the model.
 */
let latestRun = 0;

/** The files the panel's + button takes. */
const ACCEPTED = "image/*,video/mp4,video/webm,audio/*,.txt,.md,.csv,text/plain,text/markdown,text/csv";

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
  const { t, caution } = useApp();
  const { entries, message, files, internet, job, saying, failed, listenerDownload } = useStore(panelState);
  const [info, setInfo] = useState<LocalModelInfo | null>(null);
  const [listener, setListener] = useState<ListenerModel | null>(null);
  const [dropping, setDropping] = useState(false);
  const [picked, setPicked] = useState<HTMLInputElement | null>(null);
  const end = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    // Read again each time the panel opens: the model may have been chosen meanwhile.
    api.aiLocalModel().then(setInfo, (error: unknown) => change({ failed: errorNotice(t, error) }));
    api.listenerModel().then(setListener, () => setListener(null));
  }, [t]);
  // The newest of the conversation stays in view as it grows.
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "end" });
  }, [entries.length, saying, job?.step]);
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

  /**
   * The panel's files as the model gets them, their speech written down
   * first; for the questions (`making`), pictures and videos are copied
   * into the draft `draft` so the slides show them. Null when the run was
   * cancelled meanwhile.
   */
  const prepare = async (current: () => boolean, making: boolean, draft: string): Promise<Attachment[] | null> => {
    if (!info) return null;
    const attachments: Attachment[] = [];
    let textLeft = info.maxTextChars;
    for (const listed of panelState.get().files) {
      let file = listed;
      // What is said in it, written down once per language while it stays in the panel.
      if (listener?.installed && hasSound(file.kind) && file.heard?.language !== language) {
        change({ job: { step: "listening", name: file.file.name, making } });
        const sound = await soundOf(file.file, listener.rate, listener.maxSeconds);
        if (!current()) return null;
        if (!sound && file.kind === "recording") caution(t("aiPanelNoSound", { name: file.file.name }));
        const words = sound ? await api.editorTranscribe(file.file.name, language, sound) : "";
        if (!current()) return null;
        file = { ...file, heard: { language, words } };
        const heard = file.heard;
        change({ files: panelState.get().files.map((f) => (f.id === file.id ? { ...f, heard } : f)), job: { step: "preparing", making } });
      }
      let reference: string | null = null;
      if (making && (file.kind === "picture" || file.kind === "video")) {
        // The slides show the draft's own copy; copied once per draft.
        reference = file.copied?.draft === draft ? file.copied.reference : await api.editorAddDroppedMedia(draft, file.file);
        const copied = { draft, reference };
        change({ files: panelState.get().files.map((f) => (f.id === file.id ? { ...f, copied } : f)) });
      }
      let attachment: Attachment;
      try {
        attachment = await attachmentFor(file, info, reference, textLeft);
      } catch {
        if (!current()) return null;
        // A file the window cannot decode is left out, and said so.
        caution(t("aiPanelFileFailed", { name: file.file.name }));
        continue;
      }
      const content = attachment.content;
      textLeft -= content.kind === "text" ? content.text.length : content.kind === "recording" || content.kind === "video" ? content.words.length : 0;
      attachments.push(attachment);
      if (!current()) return null;
    }
    return current() ? attachments : null;
  };

  /**
   * Sends the message being typed (for the questions, `making`, also none)
   * with the conversation before it: the model's next turn, or the
   * questions, inserted as slides.
   */
  const run = async (making: boolean) => {
    const state = panelState.get();
    if (!info?.chosen || state.job) return;
    const added = unsent(state.files, state.entries);
    if (making ? !canCreate(state.message, state.files.length, state.entries) : !canSend(state.message, added.length)) return;
    if (!listener?.installed && state.files.some((file) => file.kind === "recording")) {
      caution(t("aiPanelListenerRequired"));
      return;
    }
    const draft = draftId;
    const run = ++latestRun;
    const current = () => run === latestRun;
    const prompt = state.message.trim();
    const conversation = turnsOf(state.entries);
    // What was typed joins the conversation at once, as in a chat, with the files added since the last message, which leave the message box for its bubble.
    const sent = prompt === "" && added.length === 0 ? [] : [entry({ role: "user", text: prompt, files: added.map((file) => file.id) })];
    change({ entries: [...state.entries, ...sent], pending: sent[0]?.id ?? null, message: "", job: { step: "preparing", making }, saying: "", failed: null });
    try {
      const attachments = await prepare(current, making, draft);
      // Cancelled while a file was prepared: nothing is asked.
      if (!attachments || !current()) {
        if (current()) withdraw();
        return;
      }
      change({ job: { step: "waiting", making } });
      const asked: QuestionRequest = { prompt, attachments, language, wholeQuiz: emptyQuiz, internet: panelState.get().internet, conversation, notes: panelState.get().notes };
      const progress = (step: LocalModelProgress) => {
        if (!current()) return;
        if (step.step === "said") change({ saying: panelState.get().saying + step.text, job: { ...step, making } });
        else change({ job: { ...step, making } });
      };
      if (making) {
        const made = await api.editorAskLocalModel(draft, asked, progress);
        if (!current()) return;
        const inserted = onInsert(draft, made.slides);
        if (made.webFailed) caution(t(made.sources.length > 0 ? "aiPanelWebPartly" : "aiPanelWebFailed"));
        // Said in the conversation, where the quizmaster looks; a quiz opened meanwhile is told apart.
        if (inserted === null) caution(t("aiPanelOtherQuiz"));
        else change({ entries: [...panelState.get().entries, entry({ role: "note", text: t("aiPanelInserted", { n: inserted }) })] });
      } else {
        const reply = await api.editorChatLocalModel(asked, progress);
        if (!current()) return;
        const said = entry({ role: "assistant", text: reply.text, cutOff: reply.cutOff, sources: reply.sources });
        change({ entries: [...panelState.get().entries, said], notes: reply.notes });
        if (reply.webFailed) caution(t(reply.sources.length > 0 ? "aiPanelWebPartly" : "aiPanelWebFailed"));
      }
    } catch (error) {
      if (current()) withdraw();
      if (isAppError(error) && error.code === "localModelStoppedForShow") caution(errorText(t, error));
      else if (current() && !(isAppError(error) && error.code === "localModelCancelled")) change({ failed: errorNotice(t, error) });
    } finally {
      if (current()) change({ job: null, pending: null, saying: "" });
    }
  };
  /** Takes back the unanswered message, so it can be changed and sent again rather than asked twice. */
  const withdraw = () => {
    const now = panelState.get();
    const back = now.pending === null ? null : takeBack(now.entries, now.pending, now.message);
    change({ ...back, pending: null });
  };
  const cancel = () => {
    // The panel is free again at once; files being prepared stop at the
    // next, and a request under way ends in the backend too.
    const step = job?.step;
    latestRun++;
    withdraw();
    change({ job: null, saying: "" });
    if (step === "listening") void api.editorTranscribeCancel();
    else if (step !== "preparing") void api.editorAskLocalModelCancel();
  };
  const startOver = () => {
    latestRun++;
    panelState.set({ ...START, internet: panelState.get().internet, listenerDownload: panelState.get().listenerDownload });
  };

  const chosen = info?.chosen ?? null;
  const status = job ? jobStatus(t, job) : null;
  const fresh = unsent(files, entries);
  const sendable = canSend(message, fresh.length);
  const creatable = canCreate(message, files.length, entries);
  const removeFile = (id: number) => change({ files: panelState.get().files.filter((f) => f.id !== id) });

  return (
    <aside
      className={dropping ? "ai-panel dropping" : "ai-panel"}
      aria-label={t("aiPanel")}
      onDragOver={onDragOver}
      onDragLeave={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && setDropping(false)}
      onDrop={onDrop}
    >
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
        {chosen && (
          <button type="button" className="ai-panel-icon" title={t("aiPanelNewChat")} aria-label={t("aiPanelNewChat")} disabled={job !== null || (entries.length === 0 && files.length === 0 && message === "")} onClick={startOver}>
            <EditIcon />
          </button>
        )}
        <button type="button" className="ai-panel-icon ai-panel-close" title={t("aiPanelClose")} aria-label={t("aiPanelClose")} onClick={onClose}>
          <CrossIcon />
        </button>
      </header>

      {info && !chosen ? (
        <div className="ai-panel-body">
          <div className="ai-panel-setup">
            <p>{t("aiPanelNoModel")}</p>
            <button type="button" className="browse" onClick={onChooseModel}>
              {t("aiPanelChooseModel")}
            </button>
            {failed && <NoticeBox notice={failed} />}
          </div>
        </div>
      ) : (
        <>
          <div className="ai-panel-body ai-chat" role="log" aria-label={t("aiPanelConversation")}>
            {entries.length === 0 && !job && (
              <div className="ai-welcome">
                <span className="ai-welcome-icon" aria-hidden="true">
                  <SparkleIcon />
                </span>
                <h2>{t("aiPanelWelcome")}</h2>
                <p>{t("aiPanelHint")}</p>
              </div>
            )}
            {entries.map((shown) => (
              <ChatBubble key={shown.id} entry={shown} files={files} t={t} />
            ))}
            {job && saying && (
              <div className="ai-reply writing">
                <ReplyText text={saying} />
              </div>
            )}
            {status && (
              <p className="ai-panel-status" role="status">
                <span className="ai-spinner" aria-hidden="true" />
                {status}
              </p>
            )}
            {failed && <NoticeBox notice={failed} />}
            <div ref={end} />
          </div>

          {chosen && (
            <footer className="ai-panel-footer">
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
              <div className="ai-composer">
                {fresh.length > 0 && (
                  <ul className="ai-files" aria-label={t("aiPanelFiles")}>
                    {fresh.map((file) => (
                      <FileChip key={file.id} file={file} remove={{ disabled: job !== null, label: t("aiPanelRemoveFile", { name: file.file.name }), onRemove: () => removeFile(file.id) }} />
                    ))}
                  </ul>
                )}
                <textarea
                  value={message}
                  rows={1}
                  maxLength={info?.maxPromptChars}
                  aria-label={t("aiPanelMessage")}
                  placeholder={t(entries.length === 0 ? "aiPanelMessageFirst" : "aiPanelMessageNext")}
                  disabled={job !== null}
                  onChange={(e) => change({ message: e.target.value })}
                  onKeyDown={(e) => {
                    if (keyCombo(e) === CREATE_KEYS[0]) {
                      e.preventDefault();
                      void run(true);
                    } else if (e.key === "Enter" && !e.shiftKey && !e.altKey && !e.nativeEvent.isComposing) {
                      // Enter sends, Shift+Enter starts a new line, as in a chat.
                      e.preventDefault();
                      void run(false);
                    }
                  }}
                />
                <div className="ai-composer-bar">
                  <button type="button" className="ai-panel-icon" title={t("aiPanelAttach")} aria-label={t("aiPanelAttach")} disabled={job !== null || !info} onClick={() => picked?.click()}>
                    <AddIcon />
                  </button>
                  <input
                    ref={setPicked}
                    type="file"
                    multiple
                    hidden
                    accept={ACCEPTED}
                    onChange={(e) => {
                      const chosenFiles = [...(e.target.files ?? [])];
                      e.target.value = "";
                      add(chosenFiles);
                    }}
                  />
                  <button
                    type="button"
                    className={internet ? "ai-panel-icon on" : "ai-panel-icon"}
                    aria-pressed={internet}
                    title={`${t(internet ? "aiPanelInternetOn" : "aiPanelInternetOff")}\n${t("aiPanelInternetHint")}`}
                    aria-label={t("aiPanelInternet")}
                    disabled={job !== null}
                    onClick={() => change({ internet: !internet })}
                  >
                    <GlobeIcon />
                  </button>
                  <span className="ai-composer-gap" />
                  <button type="button" className="ai-make" disabled={job !== null || !creatable} title={tooltip({ label: t("aiPanelGenerate"), keys: CREATE_KEYS })} onClick={() => void run(true)}>
                    <SparkleIcon /> {t("aiPanelGenerate")}
                  </button>
                  {job ? (
                    <button type="button" className="ai-send stop" title={t("aiPanelStop")} aria-label={t("aiPanelStop")} onClick={cancel}>
                      <StopIcon />
                    </button>
                  ) : (
                    <button type="button" className="ai-send" disabled={!sendable} title={tooltip({ label: t("aiPanelSend"), keys: ["Enter"] })} aria-label={t("aiPanelSend")} onClick={() => void run(false)}>
                      <ArrowUpIcon />
                    </button>
                  )}
                </div>
              </div>
              <p className="ai-panel-privacy">
                {t(internet ? "aiPanelPrivacyInternet" : "aiPanelPrivacy", { url: chosen.url })}{" "}
                <button type="button" className="ai-change-model" disabled={job !== null} onClick={onChooseModel}>
                  {t("aiPanelChangeModel")}
                </button>
              </p>
            </footer>
          )}
        </>
      )}
      {dropping && (
        <div className="ai-drop-cover" aria-hidden="true">
          {t("aiPanelDropHere")}
        </div>
      )}
    </aside>
  );
}

/** What the panel says while `job` is under way. */
function jobStatus(t: Translate, job: Job): string | null {
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
      return job.making ? t("aiPanelWriting", { n: job.questions }) : null;
    case "said":
      return null;
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

/**
 * One entry of the conversation, as in a chat app: the quizmaster's in a
 * bubble on the right with the files sent with it above, the model's as
 * text across the panel with a copy button, the app's notes in between.
 */
function ChatBubble({ entry: shown, files, t }: { entry: ChatEntry; files: readonly AiFile[]; t: Translate }) {
  if (shown.role === "note") return <p className="ai-note">{shown.text}</p>;
  if (shown.role === "user") {
    const sent = files.filter((file) => shown.files.includes(file.id));
    return (
      <div className="ai-asked">
        {sent.length > 0 && (
          <ul className="ai-sent-files" aria-label={t("aiPanelFiles")}>
            {sent.map((file) => (
              <FileChip key={file.id} file={file} />
            ))}
          </ul>
        )}
        {shown.text !== "" && (
          <div className="ai-bubble user">
            <p>{shown.text}</p>
          </div>
        )}
      </div>
    );
  }
  return (
    <div className="ai-reply">
      <ReplyText text={shown.text} />
      {shown.cutOff && <small>{t("aiPanelCutOff")}</small>}
      {shown.sources.length > 0 && (
        <div className="ai-sources">
          <small>{t("aiPanelSources")}</small>
          <ul>
            {shown.sources.map((source) => (
              <li key={source.url} title={source.url}>
                {source.title} <small>{hostOf(source.url)}</small>
              </li>
            ))}
          </ul>
        </div>
      )}
      <CopyButton text={shown.text} t={t} />
    </div>
  );
}

/** Copies the model's reply as it wrote it, and says so for a moment. */
function CopyButton({ text, t }: { text: string; t: Translate }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const shown = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(shown);
  }, [copied]);
  const label = t(copied ? "aiPanelCopied" : "aiPanelCopy");
  return (
    <div className="ai-reply-actions">
      <button
        type="button"
        className="ai-panel-icon"
        title={label}
        aria-label={label}
        onClick={() => {
          navigator.clipboard?.writeText(text).then(
            () => setCopied(true),
            () => {},
          );
        }}
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </button>
    </div>
  );
}

/** The model's reply with its paragraphs, headings, lists and bold text (`replyBlocks`). */
function ReplyText({ text }: { text: string }) {
  return (
    <>
      {replyBlocks(text).map((block, index) => {
        switch (block.kind) {
          case "heading":
            return (
              <h3 key={index}>
                <Runs runs={block.runs} />
              </h3>
            );
          case "list": {
            const items = block.items.map((item, at) => (
              <li key={at}>
                <Runs runs={item} />
              </li>
            ));
            return block.ordered ? (
              <ol key={index} start={block.start}>
                {items}
              </ol>
            ) : (
              <ul key={index}>{items}</ul>
            );
          }
          case "paragraph":
            return (
              <p key={index}>
                {block.lines.map((line, at) => (
                  <span key={at}>
                    {at > 0 && <br />}
                    <Runs runs={line} />
                  </span>
                ))}
              </p>
            );
        }
      })}
    </>
  );
}

function Runs({ runs }: { runs: Run[] }) {
  return runs.map((run, at) => (run.bold ? <strong key={at}>{run.text}</strong> : <span key={at}>{run.text}</span>));
}

/** A file going with the conversation: a small picture or the kind's icon, its name, and, before it is sent, a button that takes it out. */
function FileChip({ file, remove }: { file: AiFile; remove?: { disabled: boolean; label: string; onRemove: () => void } }) {
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
    <li className={`ai-file ${file.kind}`} title={file.file.name}>
      <span className="ai-file-thumb" aria-hidden="true">
        {preview ? <img src={preview} alt="" /> : file.kind === "video" ? <VideoIcon /> : file.kind === "recording" ? <SoundIcon /> : file.kind === "text" ? <TextIcon /> : <PictureIcon />}
      </span>
      <span className="ai-file-name" title={file.file.name}>
        {file.file.name}
      </span>
      {remove && (
        <button type="button" title={remove.label} aria-label={remove.label} disabled={remove.disabled} onClick={remove.onRemove}>
          <CrossIcon />
        </button>
      )}
    </li>
  );
}
