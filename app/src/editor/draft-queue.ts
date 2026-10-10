/**
 * Backend calls on the editor's draft, run one at a time in the order they
 * were made, so an older write can never land after a newer one. Writes
 * still waiting to start collapse into the latest.
 */
export class DraftQueue<T> {
  private tail: Promise<unknown> = Promise.resolve();
  /** A write that has not started yet; later writes replace its value. */
  private waiting: { value: T } | null = null;
  private readonly write: (value: T) => Promise<void>;

  /** `write` reports its own errors; a failed write does not stop the queue. */
  constructor(write: (value: T) => Promise<void>) {
    this.write = write;
  }

  /** Writes `value` after everything queued so far. */
  push(value: T): void {
    if (this.waiting) {
      this.waiting.value = value;
      return;
    }
    const slot = { value };
    this.enqueue(() => {
      if (this.waiting === slot) this.waiting = null;
      return this.write(slot.value);
    }).catch(() => undefined);
    this.waiting = slot;
  }

  /** Runs `task` after everything queued so far; its result or error is the caller's. */
  run<R>(task: () => Promise<R>): Promise<R> {
    // A write pushed from now on must come after the task.
    this.waiting = null;
    return this.enqueue(task);
  }

  private enqueue<R>(task: () => Promise<R>): Promise<R> {
    const result = this.tail.then(task);
    this.tail = result.catch(() => undefined);
    return result;
  }
}
