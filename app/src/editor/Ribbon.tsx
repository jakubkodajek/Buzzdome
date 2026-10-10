import { createContext, useContext, type MouseEvent, type ReactNode } from "react";
import { useApp } from "../app-context";
import type { StringKey } from "../i18n";
import type { Slide } from "../types";
import { tooltip, type Action, type Actions } from "./actions";
import { CategoryGallery, type CategoryGalleryProps } from "./CategoryGallery";
import { MixedContext } from "./controls";
import type { ElementId } from "./elements";
import type { PropertyRow, SlideProperties } from "./properties";
import { ThemeGallery } from "./ThemeGallery";
import { TimerGallery } from "./TimerGallery";
import type { CategoryActions } from "./useCategories";
import type { ThemeChoice } from "./useThemeChoice";

/** The ids of the properties the selected slides differ in. */
const MixedRowsContext = createContext<ReadonlySet<string>>(new Set());

export type RibbonTab = "home" | "insert" | "design" | "question" | "view" | "show" | "media";

const TABS: [Exclude<RibbonTab, "media">, StringKey][] = [
  ["home", "ribbonHome"],
  ["insert", "ribbonInsert"],
  ["design", "ribbonDesign"],
  ["question", "ribbonQuestion"],
  ["view", "ribbonView"],
  ["show", "ribbonShow"],
];

/** The contextual tab for a selected picture, video or sound. */
const MEDIA_TAB: Partial<Record<ElementId, StringKey>> = {
  picture: "mediaPicture",
  video: "mediaVideo",
  sound: "mediaSound",
};

interface RibbonProps {
  tab: RibbonTab;
  onTab: (tab: RibbonTab) => void;
  actions: Actions;
  /** Opens the layout gallery under the given button. */
  onGallery: (button: DOMRect) => void;
  slide: Slide | null;
  element: ElementId;
  /** The properties of the slide on the canvas, which set every selected slide; null without a slide. */
  properties: SlideProperties | null;
  /** The design of the clock of the slide on the canvas (`styleProperties`), which sets every selected slide; null without a slide. */
  timer: PropertyRow | null;
  /** The ids of the properties the selected slides differ in. */
  mixed: ReadonlySet<string>;
  themeChoice: ThemeChoice;
  /** The Categories view's actions, which take the place of the slides' on HOME and INSERT; null in the other views. */
  categoryView: CategoryActions | null;
  /** The categories DESIGN offers to apply to the selected slides. */
  categoryGallery: CategoryGalleryProps;
}

/** The tabbed command area at the top, as in an office suite. */
export function Ribbon({ tab, onTab, actions, onGallery, slide, element, properties, timer, mixed, themeChoice, categoryView, categoryGallery }: RibbonProps) {
  const { t } = useApp();
  const mediaTab = MEDIA_TAB[element];
  const shown = tab === "media" && !mediaTab ? "home" : tab;
  const newSlide = <BigButton action={actions.newSlide} onClick={(e) => onGallery(e.currentTarget.getBoundingClientRect())} />;

  return (
    <MixedRowsContext value={mixed}>
      <div className="ribbon">
        <nav className="ribbon-tabs" role="tablist">
          <button type="button" className="ribbon-tab file" title={`${t("ribbonFile")} (F1)`} onClick={actions.file.run}>
            {t("ribbonFile")}
          </button>
          {TABS.map(([id, label]) => (
            <button key={id} type="button" role="tab" aria-selected={shown === id} className={shown === id ? "ribbon-tab active" : "ribbon-tab"} onClick={() => onTab(id)}>
              {t(label)}
            </button>
          ))}
          {mediaTab && (
            <button
              type="button"
              role="tab"
              aria-selected={shown === "media"}
              className={shown === "media" ? "ribbon-tab contextual active" : "ribbon-tab contextual"}
              onClick={() => onTab("media")}
            >
              {t(mediaTab)}
            </button>
          )}
        </nav>
        <div className="ribbon-body" role="tabpanel">
          {shown === "home" && categoryView && (
            <>
              <Group title="groupCategories">
                <BigButton action={categoryView.newCategory} />
                <Stack actions={[categoryView.deleteCategory, categoryView.reapplyCategory]} />
              </Group>
              <Group title="groupMedia">
                <BigButton action={categoryView.categoryBackground} />
              </Group>
            </>
          )}
          {shown === "home" && !categoryView && (
            <>
              <Group title="groupClipboard">
                <BigButton action={actions.paste} />
                <Stack actions={[actions.cut, actions.copy]} />
              </Group>
              <Group title="groupSlides">
                {newSlide}
                <Stack actions={[actions.duplicate, actions.delete]} />
              </Group>
              <Group title="groupArrange">
                <Stack actions={[actions.moveUp, actions.moveDown]} />
              </Group>
              <Group title="groupEditing">
                <BigButton action={actions.edit} />
              </Group>
              <Group title="groupTools">
                <BigButton action={actions.normalizeAll} />
              </Group>
            </>
          )}
          {shown === "insert" && categoryView && (
            <>
              <Group title="groupCategories">
                <BigButton action={categoryView.newCategory} />
              </Group>
              <Group title="groupMedia">
                <BigButton action={categoryView.categoryBackground} />
              </Group>
            </>
          )}
          {shown === "insert" && !categoryView && (
            <>
              <Group title="groupSlides">{newSlide}</Group>
              <Group title="groupMedia">
                <BigButton action={actions.insertPicture} />
                <BigButton action={actions.insertVideo} />
                <BigButton action={actions.insertSound} />
                <BigButton action={actions.insertBackground} />
              </Group>
            </>
          )}
          {shown === "design" && (
            <>
              <Group title="groupThemes">
                <ThemeGallery choice={themeChoice} />
              </Group>
              {!categoryView && (
                <Group title="groupCategories">
                  <CategoryGallery {...categoryGallery} />
                  <Stack actions={[actions.saveAsCategory, { ...actions.categories, label: t("editCategories"), checked: undefined }]} />
                </Group>
              )}
            </>
          )}
          {shown === "question" && (categoryView ? <p className="ribbon-note">{t("categoryViewNote")}</p> : <QuestionTab slide={slide} properties={properties} timer={timer} />)}
          {shown === "view" && (
            <>
              <Group title="groupViews">
                <BigButton action={actions.graphical} />
                <BigButton action={actions.table} />
                <BigButton action={actions.categories} />
              </Group>
              <Group title="groupPanels">
                <BigButton action={actions.properties} />
                <BigButton action={actions.notes} />
              </Group>
            </>
          )}
          {shown === "show" && (
            <>
              <Group title="groupTest">
                <BigButton action={actions.testRun} />
                <BigButton action={actions.testRunHere} />
              </Group>
              <Group title="groupDirector">
                <BigButton action={actions.exit} />
              </Group>
            </>
          )}
          {shown === "media" && properties && (
            <>
              <Group title="groupFile">
                <BigButton action={actions.changeMedia} />
                <BigButton action={actions.removeMedia} />
              </Group>
              {element === "video" && properties.videoOptions.length > 0 && (
                <>
                  <Group title="groupPlayback">
                    <Fields rows={properties.videoOptions} />
                  </Group>
                  <Group title="groupTiming">
                    <BigButton action={actions.trimVideo} />
                    {properties.videoReplay.length > 0 && <Fields rows={properties.videoReplay} />}
                  </Group>
                  {properties.videoEffect && (
                    <Group title="groupEffects">
                      <Fields rows={[properties.videoEffect]} />
                    </Group>
                  )}
                </>
              )}
              {element === "sound" && properties.gain && (
                <>
                  <Group title="groupPlayback">
                    <Fields rows={properties.soundOptions} />
                  </Group>
                  <Group title="groupTiming">
                    <BigButton action={actions.trimSound} />
                    {properties.replay.length > 0 && <Fields rows={properties.replay} />}
                  </Group>
                  <Group title="groupEffects">
                    <BigButton action={actions.normalizeSound} />
                    <Fields rows={[properties.gain]} />
                  </Group>
                </>
              )}
            </>
          )}
        </div>
      </div>
    </MixedRowsContext>
  );
}

function QuestionTab({ slide, properties, timer }: { slide: Slide | null; properties: SlideProperties | null; timer: PropertyRow | null }) {
  const { t } = useApp();
  const mixed = useContext(MixedRowsContext);
  if (!slide || !properties || !timer) return <p className="ribbon-note">{t("noSlideSelected")}</p>;
  if (slide.type !== "question") {
    return (
      <>
        <Group title="groupSlide">
          <Fields rows={[properties.type]} />
        </Group>
        <p className="ribbon-note">{t("questionOnly")}</p>
      </>
    );
  }
  return (
    <>
      <Group title="groupSlide">
        <Fields rows={[properties.type, properties.answerTime(slide)]} />
      </Group>
      <Group title="groupAnswering">
        <Fields rows={[properties.everyoneAnswers(slide), properties.answerCount(slide), properties.correctAnswer(slide)]} />
      </Group>
      <Group title="groupPoints">
        <Fields
          rows={[
            properties.pointsStart(slide),
            properties.falling(slide),
            ...(slide.pointsEnd !== null ? [properties.pointsEnd(slide)] : []),
            properties.penalty(slide),
          ]}
        />
      </Group>
      <Group title="groupTimer">
        <TimerGallery row={timer} mixed={mixed.has(timer.id)} />
      </Group>
    </>
  );
}

function Group({ title, children }: { title: StringKey; children: ReactNode }) {
  const { t } = useApp();
  return (
    <section className="ribbon-group">
      <div className="ribbon-group-body">{children}</div>
      <h3>{t(title)}</h3>
    </section>
  );
}

/** A large button: icon above its label. */
function BigButton({ action, onClick }: { action: Action; onClick?: (event: MouseEvent<HTMLButtonElement>) => void }) {
  return (
    <button
      type="button"
      className="ribbon-big"
      title={tooltip(action)}
      aria-pressed={action.checked}
      disabled={action.disabled}
      onClick={onClick ?? action.run}
    >
      <span className="ribbon-icon">{action.icon}</span>
      <span>{action.label}</span>
    </button>
  );
}

/** Small buttons stacked in a column: icon beside the label. */
function Stack({ actions }: { actions: Action[] }) {
  return (
    <div className="ribbon-stack">
      {actions.map((action) => (
        <button
          key={action.label}
          type="button"
          className="ribbon-small"
          title={tooltip(action)}
          aria-pressed={action.checked}
          disabled={action.disabled}
          onClick={action.run}
        >
          <span className="ribbon-icon">{action.icon}</span>
          <span>{action.label}</span>
        </button>
      ))}
    </div>
  );
}

/** Labelled controls, three to a column. */
function Fields({ rows }: { rows: PropertyRow[] }) {
  const mixed = useContext(MixedRowsContext);
  return (
    <div className="ribbon-fields">
      {rows.map((row) => (
        <div key={row.id} className="ribbon-field" title={row.description}>
          <span>{row.label}</span>
          <MixedContext value={mixed.has(row.id)}>{row.control}</MixedContext>
        </div>
      ))}
    </div>
  );
}
