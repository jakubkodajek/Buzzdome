import { useMemo, useState, type MouseEvent } from "react";
import { useApp } from "../app-context";
import { sameSpan } from "../audio/clip-player";
import { normalization } from "../audio/loudness";
import { decibels } from "../format";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  BackgroundIcon,
  BackIcon,
  CheckIcon,
  CopyIcon,
  CutIcon,
  EditIcon,
  FolderIcon,
  LoudnessIcon,
  NotesIcon,
  PanelIcon,
  PasteIcon,
  PictureIcon,
  PlayIcon,
  PreviewIcon,
  RedoIcon,
  SaveIcon,
  SlideAddIcon,
  SlideIcon,
  SoundIcon,
  StopIcon,
  TableIcon,
  TrashIcon,
  TrimIcon,
  UndoIcon,
  VideoIcon,
} from "../icons";
import { isVideo, mediaFileName, MediaRootContext, mediaUrl, type MediaKind } from "../media";
import { NO_CONTEXT, slideContexts } from "../quiz";
import { ThemeContext, useResolvedTheme } from "../theme/context";
import type { Slide, ValidationError } from "../types";
import { tooltip, useActionKeys, type Action, type Actions } from "./actions";
import { Backstage, type BackstagePage } from "./Backstage";
import { ContextMenu } from "./controls";
import { DesignArea, NotesBox } from "./DesignArea";
import { EditSlideDialog } from "./EditSlideDialog";
import { isUnsaved, type OpenQuiz } from "./editor-state";
import { answerIndex, hasElement, type ElementId } from "./elements";
import { LayoutGallery } from "./LayoutGallery";
import { DEFAULT_LAYOUT, slideFromLayout, type SlideLayout } from "./layouts";
import { LoudnessDialog } from "./LoudnessDialog";
import { gainChange, LOUDNESS_STATUS_KEYS, measureSound, readLoudnessTarget } from "./normalize";
import { PropertiesPanel } from "./PropertiesPanel";
import { slideProperties, type SlideEditing } from "./properties";
import { Ribbon, type RibbonTab } from "./Ribbon";
import { duplicateSlide, freshSlideId, setBackground, setCorrect, setNotes, setPicture, setSound, setSoundOptions, setVideo } from "./slides";
import { TableView } from "./TableView";
import { Thumbnails } from "./Thumbnails";
import { TrimDialog } from "./TrimDialog";
import { useQuizEditor, type QuizEditor } from "./useQuizEditor";
import "./editor.css";

type MediaElement = "picture" | "video" | "sound";

/** How each media part of a slide is chosen and set. */
const MEDIA: Record<MediaElement, { kinds: MediaKind[]; set: (slide: Slide, file: string | null) => Slide }> = {
  picture: { kinds: ["image"], set: setPicture },
  video: { kinds: ["video"], set: setVideo },
  sound: { kinds: ["audio"], set: setSound },
};

function isMedia(element: ElementId): element is MediaElement {
  return element in MEDIA;
}

/** The selected slide, by id so it stays selected as slides move; the index is the fallback once it is gone. */
interface Selection {
  id: string | null;
  index: number;
  element: ElementId;
}

const FIRST: Selection = { id: null, index: 0, element: "slide" };

/** Where the layout gallery opens when no button opened it. */
const GALLERY_AT = { x: 180, y: 120 };

type MenuTarget = { kind: "thumbnail" } | { kind: "list" } | { kind: "canvas"; element: ElementId };

/**
 * The quiz editor in the Director's window, laid out like an office
 * suite: ribbon on top, slide thumbnails left, the slide in the middle,
 * properties right. `onTestRun` is called once a test show runs; `onExit`
 * goes back to the Director (the draft is kept).
 */
export function EditorScreen({ onTestRun, onExit }: { onTestRun: () => void; onExit: () => void }) {
  const { t } = useApp();
  const editor = useQuizEditor(onTestRun);
  if (editor.state.status === "loading") return <div className="studio studio-loading">{t("loading")}</div>;
  return <Studio editor={editor} open={editor.state.status === "open" ? editor.state : null} onExit={onExit} />;
}

function Studio({ editor, open, onExit }: { editor: QuizEditor; open: OpenQuiz | null; onExit: () => void }) {
  const { t, notify } = useApp();
  const [selection, setSelection] = useState<Selection>(FIRST);
  const [tab, setTab] = useState<RibbonTab>("home");
  const [backstage, setBackstage] = useState<BackstagePage | null>(null);
  const [view, setView] = useState<"graphical" | "table">("graphical");
  const [panel, setPanel] = useState(true);
  const [notesShown, setNotesShown] = useState(true);
  const [clipboard, setClipboard] = useState<Slide | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; target: MenuTarget } | null>(null);
  const [gallery, setGallery] = useState<{ x: number; y: number } | null>(null);
  const [dialog, setDialog] = useState<{ id: string; focus: ElementId } | null>(null);
  /** The slide whose sound is being trimmed. */
  const [trimOf, setTrimOf] = useState<string | null>(null);
  const [loudness, setLoudness] = useState(false);
  /** The slide whose background video plays in the design area. */
  const [playingBackgroundOf, setPlayingBackgroundOf] = useState<string | null>(null);
  /** The theme pointed at in the ribbon, shown on the slide until the pointer leaves. */
  const [previewTheme, setPreviewTheme] = useState<string | null>(null);

  const quiz = open?.history.present ?? null;
  const theme = useResolvedTheme(quiz?.theme ?? null);
  const slideTheme = useResolvedTheme(previewTheme ?? quiz?.theme ?? null);
  const slides = quiz?.slides ?? [];
  const contexts = useMemo(() => (quiz ? slideContexts(quiz) : []), [quiz]);
  const found = selection.id === null ? -1 : slides.findIndex((s) => s.id === selection.id);
  const index = slides.length === 0 ? -1 : found >= 0 ? found : Math.min(selection.index, slides.length - 1);
  const slide: Slide | null = slides[index] ?? null;
  const element: ElementId = slide && hasElement(slide, selection.element) ? selection.element : "slide";
  const media = isMedia(element) ? element : null;
  const dialogSlide = dialog ? slides.find((s) => s.id === dialog.id) : undefined;
  const trimSlide = trimOf === null ? undefined : slides.find((s) => s.id === trimOf);
  const backgroundVideo = slide?.background ? isVideo(slide.background.file) : false;
  const backgroundPlaying = backgroundVideo && playingBackgroundOf === slide?.id;
  const unsaved = open !== null && isUnsaved(open);

  const select = (i: number, part: ElementId = "slide") => setSelection({ id: slides[i]?.id ?? null, index: i, element: part });
  const selectPart = (part: ElementId) => setSelection((current) => ({ ...current, id: slide?.id ?? null, index, element: part }));

  const updateSlide = (id: string, change: (s: Slide) => Slide, field?: string) =>
    editor.edit({ type: "updateSlide", id, update: change }, field === undefined ? undefined : `${id}:${field}`);
  /** Asks for a media file and puts it on the slide; `part` is selected once it is there. */
  const chooseMedia = (id: string, kinds: MediaKind[], apply: (s: Slide, file: string) => Slide, part?: ElementId) =>
    void editor.pickMedia(kinds).then((file) => {
      if (!file) return;
      updateSlide(id, (s) => apply(s, file));
      if (part) setSelection((current) => (current.id === id ? { ...current, element: part } : current));
    });
  /** Measures a slide's sound and sets its loudness gain to the remembered target (SN-9). */
  const normalizeSound = (id: string) => {
    const sound = slides.find((s) => s.id === id)?.sound;
    if (!open || !sound) return;
    const target = readLoudnessTarget();
    measureSound(mediaUrl(open.mediaRoot, sound.file), sound).then(
      (measured) => {
        const change = gainChange(sound.gainDb, normalization(measured, target));
        const { gainDb } = change;
        // Only while the sound still plays the part that was measured.
        if (gainDb !== null) updateSlide(id, (s) => (s.sound?.file === sound.file && sameSpan(s.sound.part, sound.part) ? setSoundOptions(s, { gainDb }) : s));
        notify(t(change.message, { gain: decibels(gainDb ?? sound.gainDb) }));
      },
      (error: unknown) => {
        console.error("normalize:", error);
        notify(t(LOUDNESS_STATUS_KEYS.failed));
      },
    );
  };
  const editingFor = (s: Slide): SlideEditing => ({
    slide: s,
    context: contexts[slides.indexOf(s)] ?? NO_CONTEXT,
    update: (change, field) => updateSlide(s.id, change, field),
    chooseMedia: (kinds, apply) => chooseMedia(s.id, kinds, apply),
    trimSound: () => setTrimOf(s.id),
    normalizeSound: () => normalizeSound(s.id),
  });

  const insert = (inserted: Slide, at: number) => {
    editor.edit({ type: "insertSlide", index: at, slide: inserted });
    setSelection({ id: inserted.id, index: at, element: "slide" });
  };
  const insertLayout = (layout: SlideLayout) => {
    if (!quiz) return;
    const inserted = slideFromLayout(layout, freshSlideId(quiz));
    insert(inserted, index + 1);
    if (layout.media === "picture") chooseMedia(inserted.id, MEDIA.picture.kinds, setPicture, "picture");
    if (layout.media === "video") chooseMedia(inserted.id, MEDIA.video.kinds, setVideo, "video");
  };
  /** Shows a quiz that was just loaded (or the empty editor) from its start. */
  const loaded = (done: boolean) => {
    if (!done) return;
    setSelection(FIRST);
    setDialog(null);
    setBackstage(null);
  };
  /** A file with problems opens on the list of them. */
  const opened = (problems: ValidationError[] | false) => {
    loaded(problems !== false);
    if (problems && problems.length > 0) setBackstage("info");
  };
  const openEdit = (part: ElementId) => {
    if (!slide) return;
    if (isMedia(part)) chooseMedia(slide.id, MEDIA[part].kinds, MEDIA[part].set, part);
    else setDialog({ id: slide.id, focus: part });
  };
  const testRun = (from?: number) => {
    if (open && open.problems.length > 0) {
      notify(t("testRunNeedsFixes"));
      setBackstage("info");
    } else {
      void editor.testRun(from);
    }
  };

  const noSlide = slide === null;
  const actions: Actions = {
    file: { label: t("ribbonFile"), keys: ["F1"], inFields: true, run: () => setBackstage("info") },
    newQuiz: { label: t("newQuiz"), keys: ["Ctrl+N"], inFields: true, run: () => void editor.newQuiz().then(loaded) },
    openQuiz: { label: t("openFile"), icon: <FolderIcon />, keys: ["Ctrl+O"], inFields: true, run: () => setBackstage("open") },
    save: { label: t("saveQuiz"), icon: <SaveIcon />, keys: ["Ctrl+S"], inFields: true, disabled: !open, run: () => void editor.save() },
    saveAs: { label: t("saveQuizAs"), keys: ["Ctrl+Shift+S"], inFields: true, disabled: !open, run: () => void editor.saveAs() },
    closeQuiz: { label: t("closeQuiz"), disabled: !open, run: () => void editor.closeQuiz().then(loaded) },
    exit: { label: t("backToDirector"), icon: <BackIcon />, run: onExit },
    undo: {
      label: t("undo"),
      icon: <UndoIcon />,
      keys: ["Ctrl+Z"],
      inFields: true,
      repeats: true,
      disabled: !open || open.history.past.length === 0,
      run: editor.undo,
    },
    redo: {
      label: t("redo"),
      icon: <RedoIcon />,
      keys: ["Ctrl+Y", "Ctrl+Shift+Z"],
      inFields: true,
      repeats: true,
      disabled: !open || open.history.future.length === 0,
      run: editor.redo,
    },
    cut: {
      label: t("cut"),
      icon: <CutIcon />,
      keys: ["Ctrl+X"],
      disabled: noSlide,
      run: () => {
        setClipboard(slide);
        editor.edit({ type: "removeSlide", index });
      },
    },
    copy: { label: t("copy"), icon: <CopyIcon />, keys: ["Ctrl+C"], disabled: noSlide, run: () => setClipboard(slide) },
    paste: {
      label: t("paste"),
      icon: <PasteIcon />,
      keys: ["Ctrl+V"],
      disabled: !quiz || !clipboard,
      run: () => quiz && clipboard && insert(duplicateSlide(clipboard, freshSlideId(quiz)), index + 1),
    },
    delete: {
      label: t("deleteSlide"),
      icon: <TrashIcon />,
      // Delete removes what is selected: a picture, video or sound, or else the slide.
      keys: media ? undefined : ["Delete", "Backspace"],
      disabled: noSlide,
      run: () => editor.edit({ type: "removeSlide", index }),
    },
    duplicate: {
      label: t("duplicate"),
      icon: <SlideIcon />,
      keys: ["Ctrl+D"],
      disabled: noSlide,
      run: () => quiz && slide && insert(duplicateSlide(slide, freshSlideId(quiz)), index + 1),
    },
    moveUp: {
      label: t("moveUp"),
      icon: <ArrowUpIcon />,
      keys: ["Ctrl+ArrowUp"],
      disabled: index <= 0,
      run: () => editor.edit({ type: "moveSlide", from: index, to: index - 1 }),
    },
    moveDown: {
      label: t("moveDown"),
      icon: <ArrowDownIcon />,
      keys: ["Ctrl+ArrowDown"],
      disabled: noSlide || index >= slides.length - 1,
      run: () => editor.edit({ type: "moveSlide", from: index, to: index + 1 }),
    },
    edit: { label: t("editSlide"), icon: <EditIcon />, keys: ["Enter", "F2"], disabled: noSlide, run: () => openEdit(media ? "slide" : element) },
    newSlide: { label: t("newSlide"), icon: <SlideAddIcon />, disabled: !quiz, run: () => setGallery(GALLERY_AT) },
    addQuestion: { label: t("addQuestion"), keys: ["Ctrl+M"], disabled: !quiz, run: () => insertLayout(DEFAULT_LAYOUT) },
    previous: { label: t("previousSlide"), keys: ["ArrowUp", "ArrowLeft", "PageUp"], repeats: true, disabled: index <= 0, run: () => select(index - 1) },
    next: {
      label: t("nextSlide"),
      keys: ["ArrowDown", "ArrowRight", "PageDown"],
      repeats: true,
      disabled: index >= slides.length - 1,
      run: () => select(index + 1),
    },
    first: { label: t("firstSlide"), keys: ["Home"], disabled: index <= 0, run: () => select(0) },
    last: { label: t("lastSlide"), keys: ["End"], disabled: index >= slides.length - 1, run: () => select(slides.length - 1) },
    insertPicture: {
      label: t("mediaPicture"),
      icon: <PictureIcon />,
      // The end-of-round screen has the leaderboard where a picture would go.
      disabled: noSlide || slide.type === "endOfRound",
      run: () => slide && chooseMedia(slide.id, MEDIA.picture.kinds, setPicture, "picture"),
    },
    insertVideo: {
      label: t("mediaVideo"),
      icon: <VideoIcon />,
      disabled: noSlide || slide.type === "endOfRound",
      run: () => slide && chooseMedia(slide.id, MEDIA.video.kinds, setVideo, "video"),
    },
    insertSound: { label: t("mediaSound"), icon: <SoundIcon />, disabled: noSlide, run: () => slide && chooseMedia(slide.id, MEDIA.sound.kinds, setSound, "sound") },
    insertBackground: {
      label: t("mediaBackground"),
      icon: <BackgroundIcon />,
      disabled: noSlide,
      run: () => slide && chooseMedia(slide.id, ["image", "video"], setBackground),
    },
    changeMedia: {
      label: t("changeFile"),
      icon: <FolderIcon />,
      disabled: !media,
      run: () => slide && media && chooseMedia(slide.id, MEDIA[media].kinds, MEDIA[media].set, media),
    },
    removeMedia: {
      label: t("remove"),
      icon: <TrashIcon />,
      keys: media ? ["Delete", "Backspace"] : undefined,
      disabled: !media,
      run: () => slide && media && updateSlide(slide.id, (s) => MEDIA[media].set(s, null)),
    },
    trimSound: { label: t("trimAction"), icon: <TrimIcon />, disabled: !slide?.sound, run: () => slide && setTrimOf(slide.id) },
    normalizeSound: { label: t("normalizeAction"), icon: <LoudnessIcon />, disabled: !slide?.sound, run: () => slide && normalizeSound(slide.id) },
    normalizeAll: { label: t("normalizeAll"), icon: <LoudnessIcon />, disabled: !slides.some((s) => s.sound), run: () => setLoudness(true) },
    toggleBackground: {
      label: t(backgroundPlaying ? "stopBackground" : "playBackground"),
      icon: backgroundPlaying ? <StopIcon /> : <PlayIcon />,
      disabled: !backgroundVideo,
      run: () => slide && setPlayingBackgroundOf(backgroundPlaying ? null : slide.id),
    },
    removeBackground: {
      label: t("removeBackground"),
      icon: <TrashIcon />,
      disabled: !slide?.background,
      run: () => slide && updateSlide(slide.id, (s) => setBackground(s, null)),
    },
    graphical: { label: t("viewGraphical"), icon: <SlideIcon />, checked: view === "graphical", run: () => setView("graphical") },
    table: { label: t("viewTable"), icon: <TableIcon />, checked: view === "table", run: () => setView("table") },
    properties: { label: t("viewProperties"), icon: <PanelIcon />, checked: panel, run: () => setPanel(!panel) },
    notes: { label: t("viewNotes"), icon: <NotesIcon />, checked: notesShown, run: () => setNotesShown(!notesShown) },
    testRun: { label: t("testFromStart"), icon: <PlayIcon />, keys: ["F5"], inFields: true, disabled: slides.length === 0, run: () => testRun() },
    testRunHere: { label: t("testFromHere"), icon: <PreviewIcon />, keys: ["Shift+F5"], inFields: true, disabled: noSlide, run: () => testRun(index) },
  };

  const backstageShown = open === null || backstage !== null;
  const escape: Action = {
    label: "",
    keys: ["Escape"],
    inFields: true,
    run: () => (open && backstage ? setBackstage(null) : selectPart("slide")),
  };
  const all = [...Object.values(actions), escape];
  // Dialogs and menus take the keys; the FILE page only the ones that work anywhere.
  useActionKeys(dialog || menu || gallery || trimSlide || loudness ? [] : backstageShown ? all.filter((a) => a.inFields) : all);

  const menuItems = (target: MenuTarget, x: number, y: number): (Action | "separator")[] => {
    const galleryHere = { ...actions.newSlide, run: () => setGallery({ x, y }) };
    if (target.kind === "list") return [actions.paste, galleryHere];
    if (target.kind === "thumbnail") {
      return [
        actions.cut,
        actions.copy,
        actions.paste,
        "separator",
        galleryHere,
        actions.duplicate,
        actions.delete,
        "separator",
        actions.moveUp,
        actions.moveDown,
        "separator",
        actions.edit,
        actions.testRunHere,
      ];
    }
    const answer = answerIndex(target.element);
    const items: (Action | "separator")[] = [actions.edit];
    if (answer !== null && slide?.type === "question") {
      items.push({
        label: t("markCorrect"),
        icon: <CheckIcon />,
        disabled: slide.correct === answer,
        run: () => updateSlide(slide.id, (s) => (s.type === "question" ? setCorrect(s, answer) : s)),
      });
    }
    if (isMedia(target.element)) items.push(actions.changeMedia, actions.removeMedia);
    if (target.element === "sound") items.push("separator", actions.trimSound, actions.normalizeSound);
    if (target.element === "slide") {
      items.push("separator", actions.insertBackground);
      if (backgroundVideo) items.push(actions.toggleBackground);
      if (slide?.background) items.push(actions.removeBackground);
    }
    return [...items, "separator", actions.cut, actions.copy, actions.paste];
  };
  const openMenu = (event: MouseEvent, target: MenuTarget) => setMenu({ x: event.clientX, y: event.clientY, target });

  const title = quiz ? quiz.title || t("untitledQuiz") : t("quizEditor");
  const properties = slide ? slideProperties(t, editingFor(slide)) : null;

  return (
    <MediaRootContext value={open?.mediaRoot ?? null}>
      <ThemeContext value={theme}>
        <div className="studio">
          <header className="studio-titlebar">
            <div className="quick-access">
              {[actions.save, actions.undo, actions.redo].map((action) => (
                <button
                  key={action.label}
                  type="button"
                  title={tooltip(action)}
                  aria-label={action.label}
                  disabled={action.disabled}
                  onClick={action.run}
                >
                  {action.icon}
                </button>
              ))}
            </div>
            <span className="studio-title">
              {title}
              {unsaved && " *"} – {t("quizEditor")}
            </span>
            <button type="button" className="titlebar-exit" onClick={onExit}>
              <BackIcon />
              {t("backToDirector")}
            </button>
          </header>

          {backstageShown ? (
            <Backstage
              open={open}
              unsaved={unsaved}
              page={backstage ?? "info"}
              onPage={setBackstage}
              onClose={open ? () => setBackstage(null) : null}
              actions={actions}
              recentFiles={editor.recentFiles}
              onOpenFile={(path) => void editor.openQuiz(path).then(opened)}
              onTitle={(text) => editor.edit({ type: "setTitle", title: text }, "title")}
              onShowSlide={(id) => {
                setSelection({ id, index: 0, element: "slide" });
                setView("graphical");
                setBackstage(null);
              }}
            />
          ) : (
            quiz && (
              <>
                <Ribbon
                  tab={tab}
                  onTab={setTab}
                  actions={actions}
                  onGallery={(button) => setGallery({ x: button.left, y: button.bottom })}
                  slide={slide}
                  element={element}
                  properties={properties}
                  theme={theme.id}
                  onTheme={(id) => editor.edit({ type: "setTheme", theme: id })}
                  onPreviewTheme={setPreviewTheme}
                />
                <div className={panel && slide ? "studio-main with-panel" : "studio-main"}>
                  {view === "graphical" ? (
                    <>
                      <Thumbnails
                        quiz={quiz}
                        contexts={contexts}
                        selected={index}
                        problems={new Set(open.problems.flatMap((p) => (p.slideId === null ? [] : [p.slideId])))}
                        onSelect={(i) => select(i)}
                        onOpen={(i) => {
                          select(i);
                          setDialog({ id: slides[i].id, focus: "slide" });
                        }}
                        onMenu={(e, i) => openMenu(e, i === null ? { kind: "list" } : { kind: "thumbnail" })}
                        onMove={(from, to) => editor.edit({ type: "moveSlide", from, to })}
                        onAdd={actions.addQuestion.run}
                      />
                      <div className="studio-center">
                        {slide ? (
                          <ThemeContext value={slideTheme}>
                            <DesignArea
                              quiz={quiz}
                              index={index}
                              slide={slide}
                              context={contexts[index]}
                              element={element}
                              playingBackground={backgroundPlaying}
                              onSelect={selectPart}
                              onEdit={openEdit}
                              onMenu={(e, part) => openMenu(e, { kind: "canvas", element: part })}
                            />
                          </ThemeContext>
                        ) : (
                          <div className="design-empty">
                            <p>{t("noSlides")}</p>
                            <button type="button" onClick={(e) => setGallery({ x: e.clientX, y: e.clientY })}>
                              {t("newSlide")}
                            </button>
                          </div>
                        )}
                        {notesShown && slide && <NotesBox notes={slide.notes} onChange={(text) => updateSlide(slide.id, (s) => setNotes(s, text), "notes")} />}
                      </div>
                    </>
                  ) : (
                    <div className="studio-center table-center">
                      <TableView quiz={quiz} selected={index} onSelect={(i) => select(i)} onOpen={(i) => setDialog({ id: slides[i].id, focus: "slide" })} editingFor={editingFor} />
                    </div>
                  )}
                  {panel && slide && <PropertiesPanel editing={editingFor(slide)} element={element} />}
                </div>
              </>
            )
          )}

          <footer className="studio-status">
            <span>{slide ? t("slide", { n: index + 1, total: slides.length }) : t("noSlideSelected")}</span>
            {open && (
              <button type="button" className={open.problems.length > 0 ? "status-problems bad" : "status-problems"} onClick={() => setBackstage("info")}>
                {open.problems.length > 0 ? `⚠ ${t("problems", { n: open.problems.length })}` : `✓ ${t("readyToPlay")}`}
              </button>
            )}
            {unsaved && <span>{t("unsaved")}</span>}
            <span className="spacer" />
            {open?.sourcePath && <span className="status-path">{open.sourcePath}</span>}
            {[actions.graphical, actions.table].map((action) => (
              <button key={action.label} type="button" className="status-view" title={action.label} aria-pressed={action.checked} onClick={action.run}>
                {action.icon}
              </button>
            ))}
          </footer>

          {menu && <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.target, menu.x, menu.y)} onClose={() => setMenu(null)} />}
          {gallery && <LayoutGallery x={gallery.x} y={gallery.y} onPick={insertLayout} onClose={() => setGallery(null)} />}
          {open && trimSlide?.sound && (
            <TrimDialog
              key={trimSlide.id}
              url={mediaUrl(open.mediaRoot, trimSlide.sound.file)}
              name={mediaFileName(trimSlide.sound.file)}
              sound={trimSlide.sound}
              canReplay={trimSlide.type === "question"}
              onApply={(trim) => updateSlide(trimSlide.id, (s) => setSoundOptions(s, trim))}
              onClose={() => setTrimOf(null)}
            />
          )}
          {open && loudness && quiz && (
            <LoudnessDialog
              quiz={quiz}
              mediaRoot={open.mediaRoot}
              onApply={(gains) =>
                editor.edit({ type: "batch", edits: [...gains].map(([id, gainDb]) => ({ type: "updateSlide", id, update: (s: Slide) => setSoundOptions(s, { gainDb }) })) })
              }
              onClose={() => setLoudness(false)}
            />
          )}
          {dialog && dialogSlide && (
            <EditSlideDialog
              slide={dialogSlide}
              focus={dialog.focus}
              onApply={(edited) => updateSlide(edited.id, () => edited)}
              onClose={() => setDialog(null)}
            />
          )}
        </div>
      </ThemeContext>
    </MediaRootContext>
  );
}
