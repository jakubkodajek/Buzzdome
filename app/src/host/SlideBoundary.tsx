import type { ReactNode } from "react";
import { useApp } from "../app-context";
import { ErrorBoundary } from "../error-boundary";
import { WarningIcon } from "../icons";
import type { Slide } from "../types";

/**
 * Draws `slide` with `children`, a box styled by `className`. A slide that
 * fails to render shows so in that box, and the list or editor around it
 * keeps working (G23); the slide is drawn again once it changes, as when
 * the quizmaster edits it.
 */
export function SlideBoundary({ className, slide, children }: { className: string; slide: Slide; children: ReactNode }) {
  const { t } = useApp();
  return (
    <ErrorBoundary
      resetOn={slide}
      fallback={(message) => {
        const text = t("slideNotShown", { message });
        return (
          <div className={`${className} slide-failed`} role="alert" title={text}>
            <WarningIcon />
            <span>{text}</span>
          </div>
        );
      }}
    >
      {children}
    </ErrorBoundary>
  );
}
