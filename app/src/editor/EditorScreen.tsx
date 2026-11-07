import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { useApp } from "../app-context";
import { errorNotice } from "../messages";
import { NoticeBox, NotSavedBanner } from "../notice";
import {
  AddIcon,
  BackIcon,
  CategoryAddIcon,
  CategoryIcon,
  CheckIcon,
  EditIcon,
  FolderIcon,
  NotesIcon,
  PanelIcon,
  PlayIcon,
  PreviewIcon,
  RedoIcon,
  SaveIcon,
  SlideAddIcon,
  SlideIcon,
  TableIcon,
  UndoIcon,
  WarningIcon,
} from "../icons";
import { MediaRootContext } from "../media";
import { NO_CONTEXT, quizName, settingsOf, slideContexts, soundsOf, welcomeOf } from "../quiz";
import { ThemeContext } from "../theme/context";
import { useLatest, useWindowTitle } from "../hooks";
import { createStore, useStore } from "../store";
import type { StringKey } from "../i18n";
import type { Checked, EditorDraft, Slide, ValidationError } from "../types";
import type { DraftOperation } from "./draft-session";
import { actionButton, tooltip, useActionKeys, type Action, type ActionId, type Actions } from "./actions";
import { Backstage, type BackstagePage } from "./Backstage";
import { CategoryView } from "./CategoryView";
import { ContextMenu } from "./controls";
import { DesignArea, NotesBox } from "./DesignArea";
import { EditSlideDialog } from "./EditSlideDialog";
import { isUnsaved, needsInfo, type OpenQuiz } from "./editor-state";
import { isMediaElement, type ElementId } from "./elements";
import { fieldsMerge } from "./history";
import { LayoutGallery } from "./LayoutGallery";
import { NewQuizDialog, QuizStartDialog } from "./QuizStartDialog";
import { DEFAULT_LAYOUT, slideFromLayout, type SlideLayout } from "./layouts";
import { menuItems, type MenuContext, type MenuTarget } from "./menus";
import { categoryHeader, PropertiesPanel, selectionHeader } from "./PropertiesPanel";
import { slideProperties, type SlideEditing } from "./properties";
import { selectionPages } from "./property-pages";
import type { CopiedValue } from "./property-rows";
import { clickSlide, FIRST, moveToGap, selectIds } from "./selection";
import { Ribbon, type RibbonTab } from "./Ribbon";
import { formatImported, freshSlideId, setEditedTexts, setNotes, withFreshIds } from "./slides";
import { styleProperties } from "./style-properties";
import { TableView } from "./TableView";
import { Thumbnails } from "./Thumbnails";
import { useCategories } from "./useCategories";
import { draftStore, useDraftState, useQuizEditor, type QuizEditor } from "./useQuizEditor";
import { useSheetActions } from "./useSheetActions";
import { useSlideList } from "./useSlideList";
import { useSlideMedia } from "./useSlideMedia";
import { useSoundTools } from "./useSoundTools";
import { useThemeChoice } from "./useThemeChoice";
import "./editor.css";

/** Where the layout gallery opens when no button opened it. */
const GALLERY_AT = { x: 180, y: 120 };

/** A category's properties are its own: nothing is mixed. */
const NOTHING_MIXED: ReadonlySet<string> = new Set();

type View = "graphical" | "table" | "categories";

// How the editor was left, kept while it is not shown (a test run, the Director), so it opens again as it was.
const shownTab = createStore<RibbonTab>("home");
const shownView = createStore<View>("graphical");
const panelShown = createStore(true);
const notesOn = createStore(true);
const copiedValues = draftStore<CopiedValue>();

/** What the status bar says while each file operation runs. */
const BUSY_TEXT: Record<DraftOperation, StringKey> = {
  new: "busyNew",
  open: "busyOpen",
  close: "busyClose",
  save: "busySave",
  media: "busyMedia",
  import: "busyImport",
  export: "busyExport",
  testRun: "busyTestRun",
};

/** The middle of the first element shown that matches `selector`, where a menu opened by the keys appears; null when none shows. */
function centreOf(selector: string): { x: number; y: number } | null {
  const rect = document.querySelector(selector)?.getBoundingClientRect();
  return rect && rect.width > 0 ? { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 } : null;
}

/** The commands in the title bar, as an office suite's quick-access toolbar. */
const QUICK_ACCESS: ActionId[] = ["save", "undo", "redo"];

/** What the keys do in the Categories view besides the categories' own actions: nothing that changes a slide unseen. */
const CATEGORY_VIEW_KEYS: ActionId[] = ["file", "newQuiz", "openQuiz", "save", "saveAs", "closeQuiz", "undo", "redo", "findProperty", "testRun"];

/**
 * The quiz editor in the Director's window, laid out like an office
 * suite: ribbon on top, slide thumbnails left, the slide in the middle,
 * properties right. `onTestRun` is called once a test show runs; `onExit`
 * goes back to the Director (the draft is kept). With `ask` it first asks
 * what to work on: the quiz left open, a new one or a quiz file.
 */
export function EditorScreen({ ask, onTestRun, onExit }: { ask: boolean; onTestRun: () => void; onExit: () => void }) {
  const { t } = useApp();
  const editor = useQuizEditor(onTestRun);
  const { state } = editor;
  if (state.status === "loading") return <div className="studio studio-loading">{t("loading")}</div>;
  if (state.status === "unreadable") return <UnreadableDraft editor={editor} error={state.error} onExit={onExit} />;
  return <Studio editor={editor} open={state.status === "open" ? state : null} ask={ask} onExit={onExit} />;
}

/** Changes do not reach the draft on disk: over the whole editor until a write works again. */
function DraftNotWritten({ problem }: { problem: unknown }) {
  const { t } = useApp();
  const { text, detail } = errorNotice(t, problem);
  return <NotSavedBanner notice={{ text: t("draftNotWritten", { problem: text }), detail }} />;
}

/** The draft left from last time does not read: it stays until the quizmaster discards it. */
function UnreadableDraft({ editor, error, onExit }: { editor: QuizEditor; error: unknown; onExit: () => void }) {
  const { t } = useApp();
  const tryAgain = useRef<HTMLButtonElement>(null);
  useEffect(() => tryAgain.current?.focus(), []);
  useActionKeys([{ label: t("backToDirector"), keys: ["Escape"], inFields: true, run: onExit }]);
  // The heading says it does not open; what went wrong is the detail.
  const { text, detail } = errorNotice(t, error);
  return (
    <div className="studio studio-loading">
      <div className="draft-unreadable">
        <h2>{t("draftUnreadable")}</h2>
        <NoticeBox notice={{ text: detail ?? text }} />
        <p>{t("draftUnreadableInfo")}</p>
        <div className="draft-unreadable-actions">
          <button ref={tryAgain} type="button" className="primary" onClick={editor.reload}>
            {t("tryAgain")}
          </button>
          <button type="button" onClick={onExit}>
            {t("backToDirector")}
          </button>
          <button type="button" className="bad" onClick={() => void editor.discardUnreadable()}>
            {t("discardDraft")}
          </button>
        </div>
      </div>
    </div>
  );
}

function Studio({ editor, open, ask, onExit }: { editor: QuizEditor; open: OpenQuiz | null; ask: boolean; onExit: () => void }) {
  const { t } = useApp();
  /** What to work on is asked: everything (as the editor opens) or a new quiz's name. */
  const [asking, setAsking] = useState<"start" | "new" | null>(ask ? "start" : null);
  const tab = useStore(shownTab);
  const setTab = shownTab.set;
  const [backstage, setBackstage] = useState<BackstagePage | null>(null);
  const view = useStore(shownView);
  const setView = shownView.set;
  const panel = useStore(panelShown);
  const setPanel = panelShown.set;
  const notesShown = useStore(notesOn);
  const setNotesShown = notesOn.set;
  /** The value last copied from a property, to paste into another; a media file only in its own quiz. */
  const [copiedValue, setCopiedValue] = useDraftState(copiedValues, open?.draftId ?? null);
  /** Ctrl+F asked for the property search, which the panel focuses once it is shown. */
  const [findProperty, setFindProperty] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; target: MenuTarget } | null>(null);
  const [gallery, setGallery] = useState<{ x: number; y: number } | null>(null);
  const [dialog, setDialog] = useState<{ id: string; focus: ElementId } | null>(null);

  const quiz = open?.history.present ?? null;
  const { theme, canvasTheme, choice: themeChoice } = useThemeChoice(quiz?.theme ?? null, (id) =>
    editor.edit({ type: "setTheme", theme: id }),
  );
  const list = useSlideList(editor, quiz);
  const { slides, index, slide, selected, selectedSlides, element } = list;
  const categories = useCategories(editor, quiz);
  const sound = useSoundTools(open, editor.edit);
  const media = useSlideMedia(editor, list, sound);
  const contexts = useMemo(() => (quiz ? slideContexts(quiz) : []), [quiz]);
  const several = selected.length > 1;
  const dialogSlide = dialog ? slides.find((s) => s.id === dialog.id) : undefined;
  const unsaved = open !== null && isUnsaved(open);
  const { problems, warnings } = open ?? NOTHING_CHECKED;
  // Nothing to save: every change is in the quiz file that Save writes (a new quiz has none yet, a JSON quiz is saved as one).
  const saved = open !== null && !unsaved && open.savesInPlace;

  /** How the properties change slide `s`, and with it every slide of `ids`. */
  const editingFor = (s: Slide, ids: string[] = [s.id]): SlideEditing => ({
    slide: s,
    context: contexts[slides.indexOf(s)] ?? NO_CONTEXT,
    categories: categories.categories,
    update: (change, field) => list.update(ids, change, field),
    chooseMedia: (kinds, apply) => media.choose(ids, kinds, apply),
    trimSound: () => sound.trim(s.id, "sound"),
    trimVideo: () => sound.trim(s.id, "video"),
    normalizeSound: () => sound.normalize(s.id),
  });

  const insertLayout = (layout: SlideLayout) => {
    if (!quiz) return;
    const inserted = slideFromLayout(layout, freshSlideId(quiz));
    list.insert([inserted], index + 1);
    if (layout.media) media.choosePart(inserted.id, layout.media);
  };
  /** Shows a quiz that was just loaded (or the empty editor) from its start. */
  const loaded = (done: boolean) => {
    if (!done) return;
    list.setSelection(FIRST);
    setDialog(null);
    setBackstage(null);
  };
  /** A file with problems, or with what this version leaves out, opens on the list of them. */
  const opened = (draft: EditorDraft | false) => {
    loaded(draft !== false);
    if (draft && needsInfo(draft)) setBackstage("info");
  };
  const createQuiz = (name: string) =>
    void editor.newQuiz(name).then((done) => {
      loaded(done);
      if (done) setAsking(null);
    });
  const openQuiz = (recent?: string) =>
    void editor.openQuiz(recent).then((draft) => {
      opened(draft);
      if (draft) setAsking(null);
    });
  const openEdit = (part: ElementId) => {
    if (!slide) return;
    if (isMediaElement(part)) media.choosePart(slide.id, part);
    else setDialog({ id: slide.id, focus: part });
  };
  // The quiz and place as they are once a spreadsheet is read, after its dialogs.
  const latest = useLatest({ quiz, index });
  const sheet = useSheetActions(editor, quiz, (imported) => {
    const { quiz: now, index: at } = latest.current;
    if (!now) return 0;
    // A row that names a category is formatted like it.
    list.insert(withFreshIds(now, imported).map((s) => formatImported(s, now.categories ?? [])), at + 1);
    setBackstage(null);
    return imported.length;
  });
  // The backend refuses what a show cannot play (not a category's problems,
  // say); what is to fix is then listed in File › Info.
  const testRun = (from?: number) =>
    void editor.testRun(from).then((started) => {
      if (!started && problems.length > 0) setBackstage("info");
    });

  const noSlide = slide === null;
  // A file operation under way takes no other, and the editor stays open until it ends (its result lands here).
  const busy = editor.status.busy !== null;
  const actions: Actions = {
    file: { label: t("ribbonFile"), keys: ["F1", "Alt+F"], inFields: true, run: () => setBackstage("info") },
    newQuiz: { label: t("newQuiz"), keys: ["Ctrl+N"], inFields: true, disabled: busy, run: () => setAsking("new") },
    openQuiz: { label: t("openFile"), icon: <FolderIcon />, keys: ["Ctrl+O"], inFields: true, disabled: busy, run: () => setBackstage("open") },
    save: {
      label: t("saveQuiz"),
      icon: <SaveIcon />,
      keys: ["Ctrl+S"],
      inFields: true,
      appliesField: true,
      disabled: !open || saved || busy,
      note: saved ? t("quizSavedHint") : undefined,
      run: () => void editor.save(),
    },
    saveAs: { label: t("saveQuizAs"), keys: ["Ctrl+Shift+S"], inFields: true, appliesField: true, disabled: !open || busy, run: () => void editor.saveAs() },
    closeQuiz: { label: t("closeQuiz"), disabled: !open || busy, run: () => void editor.closeQuiz().then(loaded) },
    exit: { label: t("backToDirector"), icon: <BackIcon />, disabled: busy, note: busy ? t("editorBusyExit") : undefined, run: onExit },
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
    ...list.actions,
    // The slide's dialog; for a picture, video or sound the slide's too (a click on one picks a file).
    edit: { label: t("editSlide"), icon: <EditIcon />, keys: ["Enter", "F2"], disabled: noSlide, run: () => openEdit(isMediaElement(element) ? "slide" : element) },
    newSlide: { label: t("newSlide"), icon: <SlideAddIcon />, disabled: !quiz, run: () => setGallery(GALLERY_AT) },
    addQuestion: { label: t("addQuestion"), icon: <AddIcon />, keys: ["Ctrl+M"], disabled: !quiz, run: () => insertLayout(DEFAULT_LAYOUT) },
    ...media.actions,
    graphical: { label: t("viewGraphical"), icon: <SlideIcon />, checked: view === "graphical", run: () => setView("graphical") },
    table: { label: t("viewTable"), icon: <TableIcon />, checked: view === "table", run: () => setView("table") },
    categories: { label: t("viewCategories"), icon: <CategoryIcon />, checked: view === "categories", run: () => setView("categories") },
    saveAsCategory: {
      label: t("saveAsCategory"),
      icon: <CategoryAddIcon />,
      disabled: noSlide,
      run: () => {
        if (!slide) return;
        categories.create(slide);
        setView("categories");
      },
    },
    properties: { label: t("viewProperties"), icon: <PanelIcon />, checked: panel, run: () => setPanel(!panel) },
    findProperty: {
      label: t("findProperty"),
      keys: ["Ctrl+F"],
      inFields: true,
      disabled: view === "table" || (view === "graphical" ? noSlide : !categories.category),
      run: () => {
        setPanel(true);
        setFindProperty(true);
      },
    },
    notes: { label: t("viewNotes"), icon: <NotesIcon />, checked: notesShown, run: () => setNotesShown(!notesShown) },
    testRun: { label: t("testFromStart"), icon: <PlayIcon />, keys: ["F5"], inFields: true, appliesField: true, disabled: slides.length === 0 || busy, run: () => testRun() },
    testRunHere: { label: t("testFromHere"), icon: <PreviewIcon />, keys: ["Shift+F5"], inFields: true, appliesField: true, disabled: noSlide || busy || view === "categories", run: () => testRun(index) },
    importSheet: sheet.importSheet,
    sheetTemplate: sheet.sheetTemplate,
    exportSheet: sheet.exportSheet,
  };

  const backstageShown = open === null || backstage !== null;
  const escape: Action = {
    label: "",
    keys: ["Escape"],
    inFields: true,
    run: () => (open && backstage ? setBackstage(null) : view === "categories" ? categories.selectPart("slide") : list.selectPart("slide")),
  };
  // The menu key and Shift+F10 open the right click's menu: of the slide or category in the list that has the focus, else of the part selected on the canvas.
  const menuKey: Action = {
    label: "",
    keys: ["ContextMenu", "Shift+F10"],
    disabled: view === "table",
    run: () => {
      const inList = document.activeElement?.closest(".thumbnails") != null;
      const [target, place]: [MenuTarget, string] =
        view === "categories"
          ? [{ kind: "category" }, inList ? ".thumbnail.selected" : ".design-area"]
          : inList || !slide
            ? [slide ? { kind: "thumbnail" } : { kind: "list" }, slide ? ".thumbnail.selected" : ".thumbnails"]
            : [{ kind: "canvas", element }, `.design-area [data-element="${element}"]`];
      const at = centreOf(place) ?? centreOf(".design-area") ?? centreOf(".thumbnails");
      if (at) setMenu({ ...at, target });
    },
  };
  const all =
    view === "categories"
      ? [...CATEGORY_VIEW_KEYS.map((id) => actions[id]), ...Object.values(categories.actions), escape, menuKey]
      : [...Object.values(actions), escape, menuKey];
  // Dialogs and menus take the keys; the FILE page only the ones that work anywhere.
  useActionKeys(asking || dialog || menu || gallery || sound.windowOpen || sheet.windowOpen ? [] : backstageShown ? all.filter((a) => a.inFields) : all);

  const menuContext: MenuContext = {
    actions,
    categoryActions: categories.actions,
    slide,
    update: list.update,
    openGallery: (x, y) => setGallery({ x, y }),
  };
  const openMenu = (event: MouseEvent, target: MenuTarget) => setMenu({ x: event.clientX, y: event.clientY, target });

  const title = quiz ? quizName(t, quiz) : t("quizEditor");
  useWindowTitle(quiz ? `${title}${unsaved ? " *" : ""} – ${t("quizEditor")}` : t("quizEditor"), t("appTitle"));
  const editings = selectedSlides.map((s) => editingFor(s, selected));
  const shared = selectionPages(t, editings, element);
  const properties = editings.length > 0 ? slideProperties(t, editings[0]) : null;
  const timer = editings.length > 0 ? styleProperties(t, editings[0]).timer : null;
  // The panel shows the category in the Categories view, else the selected slides.
  const panelContent =
    view === "categories"
      ? categories.category && { header: categoryHeader(t, categories.category, categories.slideCount), pages: categories.pages, mixed: NOTHING_MIXED }
      : slide && { header: selectionHeader(t, slide, selectedSlides.length, element), pages: shared.pages, mixed: shared.mixed };
  const status =
    view === "categories"
      ? categories.category
        ? t("categoryNumber", { n: categories.index + 1, total: categories.categories.length })
        : t("noCategories")
      : `${slide ? t("slide", { n: index + 1, total: slides.length }) : t(slides.length === 0 ? "statusNoSlides" : "noSlideSelected")}${several ? ` · ${t("slidesSelected", { n: selected.length })}` : ""}`;
  const sameCategory = slide?.category !== undefined && selectedSlides.every((s) => s.category === slide.category);
  // The picture, video or sound tab shows only while one is selected, and goes back to HOME once none is.
  const ribbonElement = view === "categories" ? categories.element : element;
  const mediaShown = isMediaElement(ribbonElement);
  useEffect(() => {
    if (!mediaShown && shownTab.get() === "media") shownTab.set("home");
  }, [mediaShown]);

  return (
    <MediaRootContext value={open?.mediaRoot ?? null}>
      <ThemeContext value={theme}>
        {/* A file operation under way takes no other; while another quiz opens, no edits either. */}
        <div className={busy ? "studio studio-busy" : "studio"} aria-busy={busy}>
          <header className="studio-titlebar">
            <div className="quick-access">
              {QUICK_ACCESS.map((id) => (
                <button key={id} type="button" aria-label={actions[id].label} {...actionButton(actions[id])}>
                  {actions[id].icon}
                </button>
              ))}
            </div>
            {/* A long name is cut, never the unsaved mark after it. */}
            <span className="studio-title" title={title}>
              <span className="studio-title-name">{title}</span>
              {unsaved && <span className="studio-title-mark">*</span>}
              <span className="studio-title-app">– {t("quizEditor")}</span>
            </span>
            <button type="button" className="titlebar-exit" title={tooltip(actions.exit)} disabled={actions.exit.disabled} onClick={actions.exit.run}>
              <BackIcon />
              {t("backToDirector")}
            </button>
          </header>
          {editor.status.notWritten !== null && (
            <DraftNotWritten problem={editor.status.notWritten} />
          )}

          {backstageShown ? (
            <Backstage
              open={open}
              unsaved={unsaved}
              page={backstage ?? "info"}
              onPage={setBackstage}
              onClose={open ? () => setBackstage(null) : null}
              actions={actions}
              recentFiles={editor.recentFiles}
              onOpenFile={openQuiz}
              onForgetFile={editor.forgetRecentFile}
              onTitle={(text) => editor.edit({ type: "setTitle", title: text }, "title")}
              // Quick changes of one setting are one undo step, of two different ones two.
              onSettings={(settings) => quiz && editor.edit({ type: "setSettings", settings }, fieldsMerge("settings", settingsOf(quiz), settings))}
              onSounds={(sounds) => quiz && editor.edit({ type: "setSounds", sounds }, fieldsMerge("sounds", soundsOf(quiz), sounds))}
              onWelcome={(welcome) => quiz && editor.edit({ type: "setWelcome", welcome }, fieldsMerge("welcome", welcomeOf(quiz), welcome))}
              onPickMedia={(kinds) => editor.pickMedia(kinds)}
              onShowSlide={(id) => {
                list.setSelection(selectIds([id], 0));
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
                  element={ribbonElement}
                  properties={properties}
                  timer={timer}
                  mixed={shared.mixed}
                  themeChoice={themeChoice}
                  categoryView={view === "categories" ? categories.actions : null}
                  categoryGallery={{
                    categories: categories.categories,
                    current: sameCategory ? (slide?.category ?? null) : null,
                    apply: (id) => editor.edit({ type: "applyCategory", id, slides: selected }),
                  }}
                />
                <div className={panel && panelContent ? "studio-main with-panel" : "studio-main"}>
                  {view === "graphical" ? (
                    <>
                      <Thumbnails
                        quiz={quiz}
                        contexts={contexts}
                        current={index}
                        selected={new Set(selected)}
                        problems={slideIds(problems)}
                        warnings={slideIds(warnings)}
                        onSelect={(i, keys) => list.setSelection(clickSlide(list.selection, slides, i, keys))}
                        onOpen={(i) => {
                          list.select(i);
                          setDialog({ id: slides[i].id, focus: "slide" });
                        }}
                        onMenu={(e, i) => openMenu(e, i === null ? { kind: "list" } : { kind: "thumbnail" })}
                        onMove={(dragged, gap) => {
                          // A selected slide takes the others selected with it.
                          const moved = moveToGap(slides, selected.includes(dragged) ? selected : [dragged], gap);
                          if (moved) editor.edit(moved);
                        }}
                        add={actions.addQuestion}
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
                              playingBackground={media.playingBackground}
                              onSelect={list.selectPart}
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
                        {notesShown && slide && <NotesBox notes={slide.notes} onChange={(text) => list.update([slide.id], (s) => setNotes(s, text), "notes")} />}
                      </div>
                    </>
                  ) : view === "table" ? (
                    <div className="studio-center table-center">
                      <TableView quiz={quiz} selected={index} onSelect={(i) => list.select(i)} onOpen={(i) => setDialog({ id: slides[i].id, focus: "slide" })} editingFor={editingFor} />
                    </div>
                  ) : (
                    <CategoryView quiz={quiz} categories={categories} canvasTheme={canvasTheme} onMenu={(e) => openMenu(e, { kind: "category" })} />
                  )}
                  {panel && panelContent && (
                    <PropertiesPanel
                      {...panelContent}
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
            <span>{status}</span>
            {editor.status.busy && <span role="status">{t(BUSY_TEXT[editor.status.busy])}</span>}
            {open && (
              <button type="button" className={problems.length > 0 ? "status-problems bad" : warnings.length > 0 ? "status-problems warn" : "status-problems"} onClick={() => setBackstage("info")}>
                {problems.length > 0 || warnings.length > 0 ? <WarningIcon className="inline-icon" /> : <CheckIcon className="inline-icon" />}{" "}
                {problems.length > 0 ? t("problems", { n: problems.length }) : warnings.length > 0 ? t("readyWithWarnings", { n: warnings.length }) : t("readyToPlay")}
              </button>
            )}
            {open && (
              <span className="status-saved">
                {unsaved ? <WarningIcon className="inline-icon" /> : saved && <SaveIcon className="inline-icon" />} {t(unsaved ? "statusUnsaved" : saved ? "quizSaved" : "notSavedYet")}
              </span>
            )}
            <span className="spacer" />
            {/* Cut at its start: the file's name is the part that matters. */}
            {open?.sourcePath && (
              <span className="status-path" title={open.sourcePath}>
                <bdi dir="ltr">{open.sourcePath}</bdi>
              </span>
            )}
            {[actions.graphical, actions.table, actions.categories].map((action) => (
              <button key={action.label} type="button" className="status-view" title={tooltip(action)} aria-label={action.label} aria-pressed={action.checked} onClick={action.run}>
                {action.icon}
              </button>
            ))}
          </footer>

          {menu && <ContextMenu x={menu.x} y={menu.y} items={menuItems(t, menu.target, menu.x, menu.y, menuContext)} onClose={() => setMenu(null)} />}
          {gallery && <LayoutGallery x={gallery.x} y={gallery.y} onPick={insertLayout} onClose={() => setGallery(null)} />}
          {sound.windows}
          {sheet.windows}
          {asking === "start" && (
            <QuizStartDialog
              open={open}
              unsaved={unsaved}
              recentFiles={editor.recentFiles}
              busy={busy}
              onContinue={() => setAsking(null)}
              onNew={createQuiz}
              onOpen={openQuiz}
              onForget={editor.forgetRecentFile}
              onCancel={() => !busy && onExit()}
            />
          )}
          {asking === "new" && <NewQuizDialog busy={busy} onNew={createQuiz} onCancel={() => setAsking(null)} />}
          {dialog && dialogSlide && (
            <EditSlideDialog
              slide={dialogSlide}
              focus={dialog.focus}
              onApply={(opened, edited) => list.update([edited.id], (slide) => setEditedTexts(slide, opened, edited))}
              onClose={() => setDialog(null)}
            />
          )}
        </div>
      </ThemeContext>
    </MediaRootContext>
  );
}

/** What the editor shows without an open quiz. */
const NOTHING_CHECKED: Checked = { problems: [], warnings: [] };

/** The ids of the slides `problems` are on. */
function slideIds(problems: ValidationError[]): Set<string> {
  return new Set(problems.flatMap((p) => (p.slideId === null ? [] : [p.slideId])));
}
