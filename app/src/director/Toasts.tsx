import { useCallback, useEffect, useRef, useState } from "react";
import { useApp } from "../app-context";
import { useGameView } from "../hooks";
import { CrossIcon } from "../icons";
import { NoticeContent, type TonedNotice } from "../notice";

/** The most messages on screen at once; an older one makes way (`trimToasts`). */
const MAX_TOASTS = 3;
/** How long news of what was done stays on screen. */
const INFO_MS = 4000;
/** How long a failure stays at least, and at most: longer ones take longer to read. */
const ERROR_MS = { min: 7000, max: 15000, perChar: 45 };

/**
 * How long `toast` stays on screen, unless the quizmaster closes it first;
 * null until they do. During a show (`showRuns`) a failure stays: the
 * quizmaster may be looking at the hall when it comes.
 */
export function toastMs({ tone, text, detail }: TonedNotice, showRuns: boolean): number | null {
  if (tone === "info") return INFO_MS;
  if (tone === "error" && showRuns) return null;
  const chars = text.length + (detail?.length ?? 0);
  return Math.min(ERROR_MS.max, Math.max(ERROR_MS.min, chars * ERROR_MS.perChar));
}

interface Shown extends TonedNotice {
  id: number;
}

/**
 * The messages that stay when `shown` holds more than `MAX_TOASTS`. The
 * newest (last) always stays, so a message is seen however many failures
 * are open; of the others, the oldest that is not a failure makes way
 * first, so a failure that stays until closed (`toastMs`) is not pushed
 * out by news, and else the oldest failure.
 */
export function trimToasts<T extends TonedNotice>(shown: T[], max = MAX_TOASTS): T[] {
  const kept = [...shown];
  while (kept.length > Math.max(max, 1)) {
    const news = kept.slice(0, -1).findIndex((toast) => toast.tone !== "error");
    kept.splice(news === -1 ? 0 : news, 1);
  }
  return kept;
}

/** The window's messages, and `show` to add one (`AppProvider`). The same message again replaces the first. */
export function useToasts(): { toasts: Shown[]; show: (toast: TonedNotice) => void; close: (id: number) => void } {
  const [toasts, setToasts] = useState<Shown[]>([]);
  const lastId = useRef(0);
  const close = useCallback((id: number) => setToasts((shown) => shown.filter((toast) => toast.id !== id)), []);
  const show = useCallback((toast: TonedNotice) => {
    const id = ++lastId.current;
    setToasts((shown) => trimToasts([...shown.filter((old) => old.text !== toast.text || old.detail !== toast.detail), { ...toast, id }]));
  }, []);
  return { toasts, show, close };
}

/** The messages in a corner of the window, each with its own time and a button to close it. */
export function ToastStack({ toasts, close }: { toasts: Shown[]; close: (id: number) => void }) {
  const showRuns = useGameView() !== null;
  return (
    <div className="toasts">
      {toasts.map((toast) => (
        <ToastCard key={toast.id} toast={toast} ms={toastMs(toast, showRuns)} close={close} />
      ))}
    </div>
  );
}

/** A message that closes after `ms` (null: once closed), its time stopped while the pointer is on it. */
function ToastCard({ toast, ms, close }: { toast: Shown; ms: number | null; close: (id: number) => void }) {
  const { t } = useApp();
  const { id } = toast;
  const [held, setHeld] = useState(false);
  // Each message stays its own time, so a second one does not hide the first;
  // read by the pointer, it stays its whole time again from when the pointer leaves.
  useEffect(() => {
    if (ms === null || held) return;
    const timer = setTimeout(() => close(id), ms);
    return () => clearTimeout(timer);
  }, [close, id, ms, held]);
  return (
    <div
      className={`toast ${toast.tone}`}
      role={toast.tone === "error" ? "alert" : "status"}
      onPointerEnter={() => setHeld(true)}
      onPointerLeave={() => setHeld(false)}
    >
      <NoticeContent notice={toast} />
      <button type="button" className="toast-close" title={t("closeMessage")} aria-label={t("closeMessage")} onClick={() => close(id)}>
        <CrossIcon />
      </button>
    </div>
  );
}
