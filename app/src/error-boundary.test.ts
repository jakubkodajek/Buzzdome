import { describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./error-boundary";

/** A boundary drawing for `resetOn`, its `setState` recorded instead of rendering. */
function boundary(resetOn: unknown, message: string | null) {
  const shown = new ErrorBoundary({ fallback: () => null, resetOn, children: null });
  shown.state = { message };
  const setState = vi.spyOn(shown, "setState").mockImplementation(() => undefined);
  return { shown, setState };
}

describe("error boundary", () => {
  it("tries again when what it draws changes after it failed", () => {
    const { shown, setState } = boundary(2, "broken");
    shown.componentDidUpdate({ ...shown.props, resetOn: 1 }, { message: "broken" });
    expect(setState).toHaveBeenCalledWith({ message: null });
  });

  it("keeps an error caught as what it draws changed, so the part is not drawn and reported twice", () => {
    const { shown, setState } = boundary(2, "broken");
    shown.componentDidUpdate({ ...shown.props, resetOn: 1 }, { message: null });
    expect(setState).not.toHaveBeenCalled();
  });

  it("keeps an error while what it draws stays the same", () => {
    const { shown, setState } = boundary(1, "broken");
    shown.componentDidUpdate({ ...shown.props }, { message: "broken" });
    expect(setState).not.toHaveBeenCalled();
  });
});
