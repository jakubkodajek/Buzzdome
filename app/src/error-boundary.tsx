import { Component, type ReactNode } from "react";
import type { Translate } from "./i18n";

interface Props {
  /** What shows instead of `children` after one failed to render, given the error's message and a way to try again. */
  fallback: (message: string, retry: () => void) => ReactNode;
  /** Told of every render error, once. */
  onError?: (message: string) => void;
  /** Tried again by itself when this changes, such as the slide it draws. */
  resetOn?: unknown;
  children: ReactNode;
}

/**
 * Keeps a render error in one part of a window from blanking the whole
 * window (A12): the part shows `fallback` until it is tried again, `resetOn`
 * changes or the boundary is given another `key`.
 */
export class ErrorBoundary extends Component<Props, { message: string | null }> {
  state = { message: null as string | null };

  static getDerivedStateFromError(error: unknown) {
    return { message: error instanceof Error ? error.message : String(error) };
  }

  componentDidCatch(error: unknown) {
    console.error(error);
    this.props.onError?.(error instanceof Error ? error.message : String(error));
  }

  componentDidUpdate(previous: Props) {
    if (this.state.message !== null && !Object.is(previous.resetOn, this.props.resetOn)) this.setState({ message: null });
  }

  render() {
    const { message } = this.state;
    return message === null ? this.props.children : this.props.fallback(message, () => this.setState({ message: null }));
  }
}

/**
 * What a window or a part of it shows after it failed to render (A12, G25):
 * `text` says so, with a button to try again; `children` adds other ways out.
 */
export function RenderFailed({ t, text, retry, children }: { t: Translate; text: string; retry: () => void; children?: ReactNode }) {
  return (
    <div className="render-failed" role="alert">
      <p>{text}</p>
      <div className="render-failed-actions">
        <button onClick={retry}>{t("tryAgain")}</button>
        {children}
      </div>
    </div>
  );
}
