import { ANSWER_LETTERS } from "../answers";
import { useApp } from "../app-context";
import type { Translate } from "../i18n";
import { Popup } from "./controls";
import { LAYOUT_GROUPS, type SlideLayout } from "./layouts";

/** The slide layouts to insert, each drawn as a small sketch of the slide. */
export function LayoutGallery({ x, y, onPick, onClose }: { x: number; y: number; onPick: (layout: SlideLayout) => void; onClose: () => void }) {
  const { t } = useApp();
  return (
    <Popup x={x} y={y} onClose={onClose} className="layout-gallery" role="dialog" label={t("newSlide")}>
      {LAYOUT_GROUPS.map((group) => (
        <section key={group.title}>
          <h3>{t(group.title)}</h3>
          <div className="layout-tiles">
            {group.layouts.map((layout, i) => {
              const label = layoutLabel(t, layout);
              return (
                <button
                  key={i}
                  type="button"
                  className="layout-tile"
                  title={label}
                  onClick={() => {
                    onClose();
                    onPick(layout);
                  }}
                >
                  <LayoutSketch layout={layout} />
                  <span>{label}</span>
                </button>
              );
            })}
          </div>
        </section>
      ))}
    </Popup>
  );
}

/** What a layout looks like, drawn with boxes in the answer colours. */
function LayoutSketch({ layout }: { layout: SlideLayout }) {
  const answers = layout.answers ?? 0;
  return (
    <span className={answers > 0 ? "sketch has-answers" : "sketch"} aria-hidden="true">
      <span className="sketch-title" />
      {layout.media && <span className={`sketch-media sketch-${layout.media}`} />}
      {layout.type === "endOfRound" && (
        <span className="sketch-ranks">
          <span />
          <span />
          <span />
        </span>
      )}
      {answers > 0 && (
        <span className={answers > 4 ? "sketch-answers six" : "sketch-answers"}>
          {Array.from({ length: answers }, (_, i) => (
            <span key={i} className={`sketch-answer-${i}`} />
          ))}
        </span>
      )}
    </span>
  );
}

function layoutLabel(t: Translate, layout: SlideLayout): string {
  const base =
    layout.type === "question"
      ? layout.answers
        ? t("layoutAnswers", { range: `A–${ANSWER_LETTERS[layout.answers - 1]}` })
        : t("layoutSpoken")
      : t(layout.type === "billboard" ? "typeBillboard" : "typeEndOfRound");
  if (layout.media === "picture") return t("layoutWithPicture", { layout: base });
  if (layout.media === "video") return t("layoutWithVideo", { layout: base });
  return base;
}
