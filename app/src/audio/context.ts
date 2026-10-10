let context: AudioContext | null = null;

/**
 * The window's one audio context, made on first use. Browsers start a
 * context made before the user clicked anything suspended, so each use
 * asks it to run.
 */
export function audioContext(): AudioContext {
  context ??= new AudioContext();
  if (context.state === "suspended") void context.resume();
  return context;
}
