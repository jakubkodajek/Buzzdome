import { useMemo, useState, type MouseEvent } from "react";
import { useApp } from "../app-context";
import {
  BackIcon,
  CategoryIcon,
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
} from "../icons";
import { MediaRootContext } from "../media";
import { NO_CONTEXT, slideContexts } from "../quiz";
import { ThemeContext } from "../theme/context";
import { useLatest } from "../hooks";
import type { EditorDraft, Slide } from "../types";
import { tooltip, useActionKeys, type Action, type ActionId, type Actions } from "./actions";
import { Backstage, type BackstagePage } from "./Backstage";
import { CategoryView } from "./CategoryView";
import { ContextMenu } from "./controls";
import { DesignArea, NotesBox } from "./DesignArea";
import { EditSlideDialog } from "./EditSlideDialog";
import { isUnsaved, needsInfo, type OpenQuiz } from "./editor-state";
import { isMediaElement, type ElementId } from "./elements";
import { LayoutGallery } from "./LayoutGallery";
import { DEFAULT_LAYOUT, slideFromLayout, type SlideLayout } from "./layouts";
import { menuItems, type MenuContext, type MenuTarget } from "./menus";
import { categoryHeader, PropertiesPanel, selectionHeader } from "./PropertiesPanel";
import { slideProperties, type SlideEditing } from "./properties";
import { selectionPages } from "./property-pages";
import type { CopiedValue } from "./property-rows";
import { clickSlide, FIRST, moveToGap, selectIds } from "./selection";
import { Ribbon, type RibbonTab } from "./Ribbon";
import { formatImported, freshSlideId, setNotes, withFreshIds } from "./slides";
import { TableView } from "./TableView";
import { Thumbnails } from "./Thumbnails";
import { useCategories } from "./useCategories";
import { useQuizEditor, type QuizEditor } from "./useQuizEditor";
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

/** What the keys do in the Categories view besides the categories' own actions: nothing that changes a slide unseen. */
const CATEGORY_VIEW_KEYS: ActionId[] = ["file", "newQuiz", "openQuiz", "save", "saveAs", "closeQuiz", "undo", "redo", "findProperty", "testRun"];

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
  const [tab, setTab] = useState<RibbonTab>("home");
  const [backstage, setBackstage] = useState<BackstagePage | null>(null);
  const [view, setView] = useState<"graphical" | "table" | "categories">("graphical");
  const [panel, setPanel] = useState(true);
  const [notesShown, setNotesShown] = useState(true);
  /** The value last copied from a property, to paste into another. */
  const [copiedValue, setCopiedValue] = useState<CopiedValue | null>(null);
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
    ...list.actions,
    // The slide's dialog; for a picture, video or sound the slide's too (a click on one picks a file).
    edit: { label: t("editSlide"), icon: <EditIcon />, keys: ["Enter", "F2"], disabled: noSlide, run: () => openEdit(isMediaElement(element) ? "slide" : element) },
    newSlide: { label: t("newSlide"), icon: <SlideAddIcon />, disabled: !quiz, run: () => setGallery(GALLERY_AT) },
    addQuestion: { label: t("addQuestion"), keys: ["Ctrl+M"], disabled: !quiz, run: () => insertLayout(DEFAULT_LAYOUT) },
    ...media.actions,
    graphical: { label: t("viewGraphical"), icon: <SlideIcon />, checked: view === "graphical", run: () => setView("graphical") },
    table: { label: t("viewTable"), icon: <TableIcon />, checked: view === "table", run: () => setView("table") },
    categories: { label: t("viewCategories"), icon: <CategoryIcon />, checked: view === "categories", run: () => setView("categories") },
    saveAsCategory: {
      label: t("saveAsCategory"),
      icon: <CategoryIcon />,
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
    run: () => (open && backstage ? setBackstage(null) : view === "categories" ? categories.selectPart("slide") : list.selectPart("slide")),
  };
  const all =
    view === "categories"
      ? [...CATEGORY_VIEW_KEYS.map((id) => actions[id]), ...Object.values(categories.actions), escape]
      : [...Object.values(actions), escape];
  // Dialogs and menus take the keys; the FILE page only the ones that work anywhere.
  useActionKeys(dialog || menu || gallery || sound.windowOpen || sheet.windowOpen ? [] : backstageShown ? all.filter((a) => a.inFields) : all);

  const menuContext: MenuContext = {
    actions,
    categoryActions: categories.actions,
    slide,
    update: list.update,
    openGallery: (x, y) => setGallery({ x, y }),
  };
  const openMenu = (event: MouseEvent, target: MenuTarget) => setMenu({ x: event.clientX, y: event.clientY, target });

  const title = quiz ? quiz.title || t("untitledQuiz") : t("quizEditor");
  const editings = selectedSlides.map((s) => editingFor(s, selected));
  const shared = selectionPages(t, editings, element);
  const properties = editings.length > 0 ? slideProperties(t, editings[0]) : null;
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
      : `${slide ? t("slide", { n: index + 1, total: slides.length }) : t("noSlideSelected")}${several ? ` · ${t("slidesSelected", { n: selected.length })}` : ""}`;
  const sameCategory = slide?.category !== undefined && selectedSlides.every((s) => s.category === slide.category);

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
                  element={view === "categories" ? categories.element : element}
                  properties={properties}
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
                        problems={new Set(open.problems.flatMap((p) => (p.slideId === null ? [] : [p.slideId])))}
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
            {open && (
              <button type="button" className={open.problems.length > 0 ? "status-problems bad" : "status-problems"} onClick={() => setBackstage("info")}>
                {open.problems.length > 0 ? `⚠ ${t("problems", { n: open.problems.length })}` : `✓ ${t("readyToPlay")}`}
              </button>
            )}
            {unsaved && <span>{t("unsaved")}</span>}
            <span className="spacer" />
            {open?.sourcePath && <span className="status-path">{open.sourcePath}</span>}
            {[actions.graphical, actions.table, actions.categories].map((action) => (
              <button key={action.label} type="button" className="status-view" title={action.label} aria-pressed={action.checked} onClick={action.run}>
                {action.icon}
              </button>
            ))}
          </footer>

          {menu && <ContextMenu x={menu.x} y={menu.y} items={menuItems(t, menu.target, menu.x, menu.y, menuContext)} onClose={() => setMenu(null)} />}
          {gallery && <LayoutGallery x={gallery.x} y={gallery.y} onPick={insertLayout} onClose={() => setGallery(null)} />}
          {sound.windows}
          {sheet.windows}
          {dialog && dialogSlide && (
            <EditSlideDialog
              slide={dialogSlide}
              focus={dialog.focus}
              onApply={(edited) => list.update([edited.id], () => edited)}
              onClose={() => setDialog(null)}
            />
          )}
        </div>
      </ThemeContext>
    </MediaRootContext>
  );
}
