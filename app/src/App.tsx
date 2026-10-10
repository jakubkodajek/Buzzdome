import { getCurrentWindow } from "@tauri-apps/api/window";
import { Director } from "./director/Director";
import { ErrorBoundary } from "./error-boundary";
import { HostScreen, reportToDirector } from "./host/HostScreen";
import { translator, useLang } from "./i18n";

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
      <ErrorBoundary fallback={() => <div className="host-failed" />} onError={(message) => reportToDirector(t("windowFailed", { message }))}>
        <HostScreen />
      </ErrorBoundary>
    );
  }
  return (
    <ErrorBoundary
      fallback={(message, retry) => (
        <div className="window-failed" role="alert">
          <p>{t("windowFailed", { message })}</p>
          <button onClick={retry}>{t("tryAgain")}</button>
        </div>
      )}
    >
      <Director />
    </ErrorBoundary>
  );
}
