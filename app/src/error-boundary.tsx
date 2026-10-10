import { Component, type ReactNode } from "react";

interface Props {
  /** What shows instead of `children` after one failed to render, given the error's message and a way to try again. */
  fallback: (message: string, retry: () => void) => ReactNode;
  /** Told of every render error, once. */
  onError?: (message: string) => void;
  children: ReactNode;
}

/**
 * Keeps a render error in one part of a window from blanking the whole
 * window (A12): the part shows `fallback` until it is tried again, or the
 * boundary is given another `key`.
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

  render() {
    const { message } = this.state;
    return message === null ? this.props.children : this.props.fallback(message, () => this.setState({ message: null }));
  }
}
