import type { MouseEvent, ReactNode } from "react";
import { useApp } from "../app-context";
import type { StringKey } from "../i18n";
import type { Slide } from "../types";
import { tooltip, type Action, type Actions } from "./actions";
import type { ElementId } from "./elements";
import type { PropertyRow, SlideProperties } from "./properties";
import { ThemeGallery } from "./ThemeGallery";

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
  /** The selected slide's properties; null without a slide. */
  properties: SlideProperties | null;
  /** Id of the quiz's theme. */
  theme: string;
  onTheme: (id: string) => void;
  /** Shows a theme on the slide while it is pointed at; null ends that. */
  onPreviewTheme: (id: string | null) => void;
}

/** The tabbed command area at the top, as in an office suite. */
export function Ribbon({ tab, onTab, actions, onGallery, slide, element, properties, theme, onTheme, onPreviewTheme }: RibbonProps) {
  const { t } = useApp();
  const mediaTab = MEDIA_TAB[element];
  const shown = tab === "media" && !mediaTab ? "home" : tab;
  const newSlide = <BigButton action={actions.newSlide} onClick={(e) => onGallery(e.currentTarget.getBoundingClientRect())} />;

  return (
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
        {shown === "home" && (
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
        {shown === "insert" && (
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
          <Group title="groupThemes">
            <ThemeGallery current={theme} onPick={onTheme} onPreview={onPreviewTheme} />
          </Group>
        )}
        {shown === "question" && <QuestionTab slide={slide} properties={properties} />}
        {shown === "view" && (
          <>
            <Group title="groupViews">
              <BigButton action={actions.graphical} />
              <BigButton action={actions.table} />
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
              <Group title="groupPlayback">
                <Fields rows={properties.videoOptions} />
              </Group>
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
  );
}

function QuestionTab({ slide, properties }: { slide: Slide | null; properties: SlideProperties | null }) {
  const { t } = useApp();
  if (!slide || !properties) return <p className="ribbon-note">{t("noSlideSelected")}</p>;
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
  return (
    <div className="ribbon-fields">
      {rows.map((row) => (
        <div key={row.label} className="ribbon-field" title={row.description}>
          <span>{row.label}</span>
          {row.control}
        </div>
      ))}
    </div>
  );
}
