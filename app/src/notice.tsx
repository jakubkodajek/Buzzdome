import type { ReactNode } from "react";
import type { Notice } from "./messages";

/** What a notice tells: what was done, something to look at before the show, or what failed. */
export type Tone = "info" | "warn" | "error";

/** A notice with what it tells, as a toast or a box shows it. */
export interface TonedNotice extends Notice {
  tone: Tone;
}

/**
 * A notice's words, as every message in the app shows them: a small dot in
 * its tone, what happened, and under it, smaller, the system's words or
 * the file it was about (`Notice.detail`). The box around it, if any,
 * gives the tone (`--tone`).
 */
export function NoticeContent({ notice }: { notice: Notice }) {
  return (
    <>
      <ToneDot />
      <NoticeText notice={notice} />
    </>
  );
}

/** The small dot in the tone (`--tone`) of what it stands before: a notice, or a state line such as the receiver's. */
export function ToneDot() {
  return <span className="notice-dot" aria-hidden="true" />;
}

function NoticeText({ notice }: { notice: Notice }) {
  return (
    <div className="notice-body">
      <p>{notice.text}</p>
      {notice.detail && <small>{notice.detail}</small>}
    </div>
  );
}

/**
 * A notice kept where it applies until what it says changes; `children`
 * are its ways out, such as buttons.
 */
export function NoticeBox({ notice, tone = "error", children }: { notice: Notice; tone?: Tone; children?: ReactNode }) {
  return (
    <div className={`notice ${tone}`} role={tone === "error" ? "alert" : "status"}>
      <NoticeContent notice={notice} />
      {children && <div className="notice-actions">{children}</div>}
    </div>
  );
}

/** Saving does not work, the show's journal or the editor's draft: a bar over the whole window until it does. */
export function NotSavedBanner({ notice }: { notice: Notice }) {
  return (
    <div className="not-saved-banner" role="alert">
      <NoticeText notice={notice} />
    </div>
  );
}
