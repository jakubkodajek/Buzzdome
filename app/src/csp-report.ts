// What the content security policy blocks reaches only the webview's
// console, which nobody sees during a show. This puts it in the app's log,
// which the quizmaster sends when something went wrong.

/** The most blocks noted in a minute: a page that keeps trying would push the rest out of the log. */
export const MAX_NOTED_PER_MINUTE = 5;
const MINUTE_MS = 60_000;

/** What a block tells: `timeStamp` is when it happened, on the page's own clock. */
export type Violation = Pick<SecurityPolicyViolationEvent, "effectiveDirective" | "blockedURI" | "sourceFile" | "lineNumber" | "timeStamp">;

/**
 * A listener that hands `note` a line for each block, at most
 * `MAX_NOTED_PER_MINUTE` a minute. The next line after a minute that had
 * more says how many were left out.
 */
export function blockNoter(note: (line: string) => void): (violation: Violation) => void {
  let minuteFrom = -Infinity;
  let noted = 0;
  let leftOut = 0;
  return (violation) => {
    if (violation.timeStamp - minuteFrom >= MINUTE_MS) {
      minuteFrom = violation.timeStamp;
      noted = 0;
    }
    if (noted === MAX_NOTED_PER_MINUTE) {
      leftOut += 1;
      return;
    }
    noted += 1;
    const where = violation.sourceFile ? ` at ${violation.sourceFile}:${violation.lineNumber}` : "";
    const before = leftOut > 0 ? ` (${leftOut} more left out before it)` : "";
    leftOut = 0;
    note(`the content security policy blocked ${violation.blockedURI || "a resource"} (${violation.effectiveDirective})${where}${before}`);
  };
}

/** Notes the blocks in `target`, a window, through `note`. */
export function installBlockNotes(target: Pick<Window, "addEventListener">, note: (line: string) => void) {
  target.addEventListener("securitypolicyviolation", blockNoter(note));
}
