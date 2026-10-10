import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { api } from "./api";
import { confirmAction } from "./confirm";
import { Director } from "./director/Director";
import { ErrorBoundary, RenderFailed } from "./error-boundary";
import { useBackendEvent } from "./hooks";
import { HostScreen, reportToDirector } from "./host/HostScreen";
import { translator, useLang, type Translate } from "./i18n";
import { KEYS } from "./keyboard";

// Both windows load the same bundle; the window label picks the screen.
const isHost = getCurrentWindow().label === "host";

/**
 * The window's screen. Should it fail to render, the projector goes black
 * and tells the Director, and the Director offers to try again (A12).
 */
export default function App() {
  const [lang] = useLang();
  const t = translator(lang);
  if (isHost) {
    return (
      <ErrorBoundary fallback={() => <HostFailed />} onError={(message) => reportToDirector(t("projectorFailed", { message }))}>
        <HostScreen />
      </ErrorBoundary>
    );
  }
  return <DirectorWindow t={t} />;
}

/**
 * The Director, which can be closed during a show even once its screen
 * failed to render (G3): the close request is answered here, above the
 * screen's boundary, and the fallback can close the app itself.
 */
function DirectorWindow({ t }: { t: Translate }) {
  // Closing this window during a show asks first; the app ends with it (A10).
  useBackendEvent("close-requested", () => {
    // A dialog that does not open leaves the request unanswered, so the next close goes through.
    confirmAction(t, t("quitDuringShow"))
      .then((quit) => (quit ? api.quitApp() : api.closeDeclined()))
      .catch((error: unknown) => console.error(error));
  });
  return (
    <ErrorBoundary
      fallback={(message, retry) => (
        <RenderFailed t={t} text={t("windowFailed", { message })} retry={retry}>
          <button onClick={() => void api.quitApp()}>{t("closeAppKeepShow")}</button>
        </RenderFailed>
      )}
    >
      <Director />
    </ErrorBoundary>
  );
}

/** The projector after its screen failed to render: black, and M or Esc still closes it (G24). */
function HostFailed() {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      if (event.key === "Escape" || event.key.toLowerCase() === KEYS.projector) void api.closeHostWindow();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return <div className="host-failed" />;
}
