import type { ReactNode } from "react";
import { CheckIcon, WarningIcon } from "./icons";
import type { Notice } from "./messages";

/** What a notice tells: what was done, something to look at before the show, or what failed. */
export type Tone = "info" | "warn" | "error";

/** A notice with what it tells, as a toast or a box shows it. */
export interface TonedNotice extends Notice {
  tone: Tone;
}

/**
 * A notice's sign and words, as every message in the app shows them: what
 * happened first, and under it, smaller, the system's words or the file it
 * was about (`Notice.detail`).
 */
export function NoticeContent({ notice, tone }: { notice: Notice; tone: Tone }) {
  return (
    <>
      <span className="notice-icon" aria-hidden="true">
        {tone === "info" ? <CheckIcon /> : <WarningIcon />}
      </span>
      <div className="notice-body">
        <p>{notice.text}</p>
        {notice.detail && <small>{notice.detail}</small>}
      </div>
    </>
  );
}

/**
 * A notice kept where it applies until what it says changes; `children`
 * are its ways out, such as buttons.
 */
export function NoticeBox({ notice, tone = "error", children }: { notice: Notice; tone?: Tone; children?: ReactNode }) {
  return (
    <div className={`notice ${tone}`} role={tone === "error" ? "alert" : "status"}>
      <NoticeContent notice={notice} tone={tone} />
      {children && <div className="notice-actions">{children}</div>}
    </div>
  );
}

/** Saving does not work, the show's journal or the editor's draft: a bar over the whole window until it does. */
export function NotSavedBanner({ notice }: { notice: Notice }) {
  return (
    <div className="not-saved-banner" role="alert">
      <NoticeContent notice={notice} tone="error" />
    </div>
  );
}
