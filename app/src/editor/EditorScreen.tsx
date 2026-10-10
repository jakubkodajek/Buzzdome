import { useMemo, useState, type MouseEvent } from "react";
import { useApp } from "../app-context";
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
import { isVideo, MediaRootContext, type MediaKind } from "../media";
import { NO_CONTEXT, showsRanking, slideContexts } from "../quiz";
import { ThemeContext } from "../theme/context";
import { useLatest } from "../hooks";
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
import { PropertiesPanel } from "./PropertiesPanel";
import { slideProperties, type SlideEditing } from "./properties";
import { selectionPages } from "./property-pages";
import type { CopiedValue } from "./property-rows";
import {
  clickSlide,
  FIRST,
  insertSlides,
  moveByOne,
  moveToGap,
  positions,
  removeSlides,
  selectAll,
  selectedIds,
  selectIds,
  selectOne,
  type Selection,
} from "./selection";
import { Ribbon, type RibbonTab } from "./Ribbon";
import {
  freshSlideId,
  setBackground,
  setCorrect,
  setNotes,
  setPicture,
  setSound,
  setVideo,
  withFreshIds,
} from "./slides";
import { TableView } from "./TableView";
import { Thumbnails } from "./Thumbnails";
import { useQuizEditor, type QuizEditor } from "./useQuizEditor";
import { useSheetActions } from "./useSheetActions";
import { useSoundTools } from "./useSoundTools";
import { useThemeChoice } from "./useThemeChoice";
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
  const [clipboard, setClipboard] = useState<Slide[] | null>(null);
  /** The value last copied from a property, to paste into another. */
  const [copiedValue, setCopiedValue] = useState<CopiedValue | null>(null);
  /** Ctrl+F asked for the property search, which the panel focuses once it is shown. */
  const [findProperty, setFindProperty] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; target: MenuTarget } | null>(null);
  const [gallery, setGallery] = useState<{ x: number; y: number } | null>(null);
  const [dialog, setDialog] = useState<{ id: string; focus: ElementId } | null>(null);
  /** The slide whose sound or video is being trimmed. */
  /** The slide whose background video plays in the design area. */
  const [playingBackgroundOf, setPlayingBackgroundOf] = useState<string | null>(null);

  const quiz = open?.history.present ?? null;
  const { theme, canvasTheme, choice: themeChoice } = useThemeChoice(quiz?.theme ?? null, (id) =>
    editor.edit({ type: "setTheme", theme: id }),
  );
  const slides = quiz?.slides ?? [];
  const contexts = useMemo(() => (quiz ? slideContexts(quiz) : []), [quiz]);
  const found = selection.id === null ? -1 : slides.findIndex((s) => s.id === selection.id);
  const index = slides.length === 0 ? -1 : found >= 0 ? found : Math.min(selection.index, slides.length - 1);
  const slide: Slide | null = slides[index] ?? null;
  const selected = selectedIds(selection, slides);
  const several = selected.length > 1;
  /** The selected slides, the one on the canvas first. */
  const selectedSlides = slide ? [slide, ...slides.filter((s) => s !== slide && selected.includes(s.id))] : [];
  const element: ElementId = slide && !several && hasElement(slide, selection.element) ? selection.element : "slide";
  const media = isMedia(element) ? element : null;
  const dialogSlide = dialog ? slides.find((s) => s.id === dialog.id) : undefined;
  const backgroundVideo = slide?.background ? isVideo(slide.background.file) : false;
  const backgroundPlaying = backgroundVideo && playingBackgroundOf === slide?.id;
  const unsaved = open !== null && isUnsaved(open);

  const select = (i: number, part: ElementId = "slide") => setSelection(selectOne(slides, i, part));
  /** Selects a part of the slide on the canvas, which leaves only that slide selected. */
  const selectPart = (part: ElementId) => setSelection(selectOne(slides, index, part));

  /** Changes the slides `ids` as one undo step; quick changes to one `field` of them merge. */
  const updateSlides = (ids: string[], change: (s: Slide) => Slide, field?: string) =>
    editor.edit(
      ids.length === 1 ? { type: "updateSlide", id: ids[0], update: change } : { type: "batch", edits: ids.map((id) => ({ type: "updateSlide", id, update: change })) },
      field === undefined ? undefined : `${ids.join(",")}:${field}`,
    );
  const updateSlide = (id: string, change: (s: Slide) => Slide, field?: string) => updateSlides([id], change, field);
  /** Asks for a media file and puts it on the slides; `part` is selected once it is there (one slide). */
  const chooseMedia = (ids: string[], kinds: MediaKind[], apply: (s: Slide, file: string) => Slide, part?: ElementId) =>
    void editor.pickMedia(kinds).then((file) => {
      if (!file) return;
      updateSlides(ids, (s) => apply(s, file));
      if (part && ids.length === 1) setSelection((current) => (current.id === ids[0] ? { ...current, element: part } : current));
    });
  const sound = useSoundTools(open, editor.edit);
  /** How the properties change slide `s`, and with it every slide of `ids`. */
  const editingFor = (s: Slide, ids: string[] = [s.id]): SlideEditing => ({
    slide: s,
    context: contexts[slides.indexOf(s)] ?? NO_CONTEXT,
    update: (change, field) => updateSlides(ids, change, field),
    chooseMedia: (kinds, apply) => chooseMedia(ids, kinds, apply),
    trimSound: () => sound.trim(s.id, "sound"),
    trimVideo: () => sound.trim(s.id, "video"),
    normalizeSound: () => sound.normalize(s.id),
  });

  /** Inserts slides at `at` as one change and selects them. */
  const insert = (inserted: Slide[], at: number) => {
    editor.edit(insertSlides(inserted, at));
    setSelection(selectIds(inserted.map((s) => s.id), at));
  };
  /** Removes the selected slides as one change; the slide after them is selected. */
  const removeSelected = () => {
    editor.edit(removeSlides(positions(slides, selected)));
    setSelection(selectOne(slides, positions(slides, selected)[0] ?? index));
  };
  const insertLayout = (layout: SlideLayout) => {
    if (!quiz) return;
    const inserted = slideFromLayout(layout, freshSlideId(quiz));
    insert([inserted], index + 1);
    if (layout.media === "picture") chooseMedia([inserted.id], MEDIA.picture.kinds, setPicture, "picture");
    if (layout.media === "video") chooseMedia([inserted.id], MEDIA.video.kinds, setVideo, "video");
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
    if (isMedia(part)) chooseMedia([slide.id], MEDIA[part].kinds, MEDIA[part].set, part);
    else setDialog({ id: slide.id, focus: part });
  };
  // The quiz and place as they are once a spreadsheet is read, after its dialogs.
  const latest = useLatest({ quiz, index });
  const sheet = useSheetActions(editor, quiz, (imported) => {
    const { quiz: now, index: at } = latest.current;
    if (!now) return 0;
    insert(withFreshIds(now, imported), at + 1);
    setBackstage(null);
    return imported.length;
  });
  const testRun = (from?: number) => {
    if (open && open.problems.length > 0) {
      notify(t("testRunNeedsFixes"));
      setBackstage("info");
    } else {
      void editor.testRun(from);
    }
  };

  const noSlide = slide === null;
  const selectedSlidesInOrder = slides.filter((s) => selected.includes(s.id));
  const moveUp = moveByOne(slides, selected, -1);
  const moveDown = moveByOne(slides, selected, 1);
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
        setClipboard(selectedSlidesInOrder);
        removeSelected();
      },
    },
    copy: { label: t("copy"), icon: <CopyIcon />, keys: ["Ctrl+C"], disabled: noSlide, run: () => setClipboard(selectedSlidesInOrder) },
    paste: {
      label: t("paste"),
      icon: <PasteIcon />,
      keys: ["Ctrl+V"],
      disabled: !quiz || !clipboard,
      run: () => quiz && clipboard && insert(withFreshIds(quiz, clipboard), index + 1),
    },
    delete: {
      label: t("deleteSlide"),
      icon: <TrashIcon />,
      // Delete removes what is selected: a picture, video or sound, or else the slides.
      keys: media ? undefined : ["Delete", "Backspace"],
      disabled: noSlide,
      run: removeSelected,
    },
    duplicate: {
      label: t("duplicate"),
      icon: <SlideIcon />,
      keys: ["Ctrl+D"],
      disabled: noSlide,
      // The copies go after the last selected slide.
      run: () => quiz && insert(withFreshIds(quiz, selectedSlidesInOrder), Math.max(...positions(slides, selected)) + 1),
    },
    moveUp: {
      label: t("moveUp"),
      icon: <ArrowUpIcon />,
      keys: ["Ctrl+ArrowUp"],
      disabled: moveUp === null,
      run: () => moveUp && editor.edit(moveUp),
    },
    moveDown: {
      label: t("moveDown"),
      icon: <ArrowDownIcon />,
      keys: ["Ctrl+ArrowDown"],
      disabled: moveDown === null,
      run: () => moveDown && editor.edit(moveDown),
    },
    selectAll: {
      label: t("selectAllSlides"),
      keys: ["Ctrl+A"],
      disabled: slides.length < 2,
      run: () => setSelection(selectAll(selection, slides, index)),
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
      // Ranking screens have the ranking where a picture would go.
      disabled: noSlide || showsRanking(slide.type),
      run: () => slide && chooseMedia([slide.id], MEDIA.picture.kinds, setPicture, "picture"),
    },
    insertVideo: {
      label: t("mediaVideo"),
      icon: <VideoIcon />,
      disabled: noSlide || showsRanking(slide.type),
      run: () => slide && chooseMedia([slide.id], MEDIA.video.kinds, setVideo, "video"),
    },
    insertSound: { label: t("mediaSound"), icon: <SoundIcon />, disabled: noSlide, run: () => slide && chooseMedia([slide.id], MEDIA.sound.kinds, setSound, "sound") },
    insertBackground: {
      label: t("mediaBackground"),
      icon: <BackgroundIcon />,
      disabled: noSlide,
      // A background goes on every selected slide.
      run: () => chooseMedia(selected, ["image", "video"], setBackground),
    },
    changeMedia: {
      label: t("changeFile"),
      icon: <FolderIcon />,
      disabled: !media,
      run: () => slide && media && chooseMedia([slide.id], MEDIA[media].kinds, MEDIA[media].set, media),
    },
    removeMedia: {
      label: t("remove"),
      icon: <TrashIcon />,
      keys: media ? ["Delete", "Backspace"] : undefined,
      disabled: !media,
      run: () => slide && media && updateSlide(slide.id, (s) => MEDIA[media].set(s, null)),
    },
    trimSound: { label: t("trimAction"), icon: <TrimIcon />, disabled: !slide?.sound, run: () => slide && sound.trim(slide.id, "sound") },
    trimVideo: { label: t("trimAction"), icon: <TrimIcon />, disabled: !slide?.video, run: () => slide && sound.trim(slide.id, "video") },
    normalizeSound: { label: t("normalizeAction"), icon: <LoudnessIcon />, disabled: !slide?.sound, run: () => slide && sound.normalize(slide.id) },
    normalizeAll: { label: t("normalizeAll"), icon: <LoudnessIcon />, disabled: !slides.some((s) => s.sound), run: sound.normalizeAll },
    toggleBackground: {
      label: t(backgroundPlaying ? "stopBackground" : "playBackground"),
      icon: backgroundPlaying ? <StopIcon /> : <PlayIcon />,
      disabled: !backgroundVideo,
      run: () => slide && setPlayingBackgroundOf(backgroundPlaying ? null : slide.id),
    },
    removeBackground: {
      label: t("removeBackground"),
      icon: <TrashIcon />,
      disabled: !selectedSlides.some((s) => s.background),
      run: () => updateSlides(selected, (s) => setBackground(s, null)),
    },
    graphical: { label: t("viewGraphical"), icon: <SlideIcon />, checked: view === "graphical", run: () => setView("graphical") },
    table: { label: t("viewTable"), icon: <TableIcon />, checked: view === "table", run: () => setView("table") },
    properties: { label: t("viewProperties"), icon: <PanelIcon />, checked: panel, run: () => setPanel(!panel) },
    findProperty: {
      label: t("findProperty"),
      keys: ["Ctrl+F"],
      inFields: true,
      disabled: noSlide || view !== "graphical",
      run: () => {
        setPanel(true);
        setFindProperty(true);
      },
    },
    notes: { label: t("viewNotes"), icon: <NotesIcon />, checked: notesShown, run: () => setNotesShown(!notesShown) },
    testRun: { label: t("testFromStart"), icon: <PlayIcon />, keys: ["F5"], inFields: true, disabled: slides.length === 0, run: () => testRun() },
    testRunHere: { label: t("testFromHere"), icon: <PreviewIcon />, keys: ["Shift+F5"], inFields: true, disabled: noSlide, run: () => testRun(index) },
    importSheet: sheet.importSheet,
    sheetTemplate: sheet.sheetTemplate,
    exportSheet: sheet.exportSheet,
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
  useActionKeys(dialog || menu || gallery || sound.windowOpen || sheet.windowOpen ? [] : backstageShown ? all.filter((a) => a.inFields) : all);

  const menuItems = (target: MenuTarget, x: number, y: number): (Action | "separator")[] => {
    const galleryHere = { ...actions.newSlide, run: () => setGallery({ x, y }) };
    if (target.kind === "list") return [actions.paste, galleryHere];
    if (target.kind === "thumbnail") {
      return [
        actions.cut,
        actions.copy,
        actions.paste,
        actions.selectAll,
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
    if (target.element === "video") items.push("separator", actions.trimVideo);
    if (target.element === "slide") {
      items.push("separator", actions.insertBackground);
      if (backgroundVideo) items.push(actions.toggleBackground);
      if (slide?.background) items.push(actions.removeBackground);
    }
    return [...items, "separator", actions.cut, actions.copy, actions.paste];
  };
  const openMenu = (event: MouseEvent, target: MenuTarget) => setMenu({ x: event.clientX, y: event.clientY, target });

  const title = quiz ? quiz.title || t("untitledQuiz") : t("quizEditor");
  const editings = selectedSlides.map((s) => editingFor(s, selected));
  const shared = selectionPages(t, editings, element);
  const properties = editings.length > 0 ? slideProperties(t, editings[0]) : null;

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
                setSelection(selectIds([id], 0));
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
                  mixed={shared.mixed}
                  themeChoice={themeChoice}
                />
                <div className={panel && slide ? "studio-main with-panel" : "studio-main"}>
                  {view === "graphical" ? (
                    <>
                      <Thumbnails
                        quiz={quiz}
                        contexts={contexts}
                        current={index}
                        selected={new Set(selected)}
                        problems={new Set(open.problems.flatMap((p) => (p.slideId === null ? [] : [p.slideId])))}
                        onSelect={(i, keys) => setSelection(clickSlide(selection, slides, i, keys))}
                        onOpen={(i) => {
                          select(i);
                          setDialog({ id: slides[i].id, focus: "slide" });
                        }}
                        onMenu={(e, i) => openMenu(e, i === null ? { kind: "list" } : { kind: "thumbnail" })}
                        onMove={(dragged, gap) => {
                          // A selected slide takes the others selected with it.
                          const moved = moveToGap(slides, selected.includes(dragged) ? selected : [dragged], gap);
                          if (moved) editor.edit(moved);
                        }}
                        onAdd={actions.addQuestion.run}
                      />
                      <div className="studio-center">
                        {slide ? (
                          <ThemeContext value={canvasTheme}>
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
                  {panel && slide && (
                    <PropertiesPanel
                      slide={slide}
                      count={selectedSlides.length}
                      element={element}
                      pages={shared.pages}
                      mixed={shared.mixed}
                      copied={copiedValue}
                      onCopy={setCopiedValue}
                      focusSearch={findProperty}
                      onSearchFocused={() => setFindProperty(false)}
                    />
                  )}
                </div>
              </>
            )
          )}

          <footer className="studio-status">
            <span>
              {slide ? t("slide", { n: index + 1, total: slides.length }) : t("noSlideSelected")}
              {several && ` · ${t("slidesSelected", { n: selected.length })}`}
            </span>
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
          {sound.windows}
          {sheet.windows}
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
