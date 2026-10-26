import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { api } from "./api";
import { confirmAction } from "./confirm";
import { Director } from "./director/Director";
import { ErrorBoundary, RenderFailed } from "./error-boundary";
import { useBackendEvent } from "./hooks";
import { HostScreen, reportToDirector } from "./host/HostScreen";
import { menuWords, translator, useLang, type Lang, type Translate } from "./i18n";
import { useProjectorCloseKeys } from "./keyboard";
import { thrownText } from "./messages";

// Both windows load the same bundle; the window label picks the screen.
const isHost = getCurrentWindow().label === "host";

/** A problem met above the window's screen, which has nowhere to show it: it goes in the app's log. */
const logProblem = (error: unknown) => {
  console.error(error);
  api.logProblem(thrownText(error)).catch(() => undefined);
};

/**
 * The window's screen. Should it fail to render, the projector goes black
 * and tells the Director, and the Director offers to try again.
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
  return <DirectorWindow lang={lang} t={t} />;
}

/**
 * The Director, which can be closed during a show even once its screen
 * failed to render: the close request is answered here, above the
 * screen's boundary, and the fallback can close the app itself.
 */
function DirectorWindow({ lang, t }: { lang: Lang; t: Translate }) {
  // Closing this window during a show asks first; the app ends with it.
  useBackendEvent("close-requested", () => {
    // A dialog that does not open leaves the request unanswered, so a close in the next minute goes through.
    confirmAction(t, t("quitDuringShow"))
      .then((quit) => (quit ? api.quitApp() : api.closeDeclined()))
      .catch(logProblem);
  });
  // The macOS app menu in the UI's language.
  useEffect(() => {
    api.setMenuWords(menuWords(translator(lang))).catch(logProblem);
  }, [lang]);
  return (
    <ErrorBoundary
      fallback={(message, retry) => (
        <RenderFailed t={t} text={t("windowFailed", { message })} retry={retry}>
          <button onClick={() => api.quitApp().catch(logProblem)}>{t("closeAppKeepShow")}</button>
        </RenderFailed>
      )}
    >
      <Director />
    </ErrorBoundary>
  );
}

/** Closes the projector whose screen failed. */
const closeFailedHost = () => {
  api.closeHostWindow().catch(logProblem);
};

/** The projector after its screen failed to render: black, and M or Esc still closes it. */
function HostFailed() {
  useProjectorCloseKeys(closeFailedHost);
  return <div className="host-failed" />;
}
