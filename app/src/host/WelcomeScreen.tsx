import { useState } from "react";
import { DEFAULT_WELCOME } from "../bindings/Constants";
import { isVideo, useMediaUrl } from "../media";
import { Picture, type ScreenMode } from "../slide-media";
import type { GameView } from "../types";

/**
 * The welcome screen the show opens on (spec SF-3): the heading by the
 * quiz's logo, the intro picture or video in the middle, and the footer on
 * a strip, over the theme's welcome background. In the show the video
 * plays over and over without its sound, as the welcome music plays.
 */
export function WelcomeScreen({ view, mode }: { view: GameView; mode: ScreenMode }) {
  const welcome = view.welcome ?? DEFAULT_WELCOME;
  const toUrl = useMediaUrl();
  const logo = toUrl(welcome.logo);
  const intro = toUrl(welcome.intro);
  const video = welcome.intro !== null && isVideo(welcome.intro);
  return (
    <div className="welcome welcome-screen">
      <header className="welcome-header">
        {logo && (
          <span className="welcome-logo">
            <Picture key={logo} src={logo} />
          </span>
        )}
        <span className="welcome-heading">{welcome.heading || view.quizTitle}</span>
      </header>
      <div className="welcome-body">
        {intro && <div className="welcome-intro">{video ? <IntroVideo key={intro} src={intro} mode={mode} /> : <Picture key={intro} src={intro} />}</div>}
      </div>
      {welcome.footer && <footer className="welcome-footer">{welcome.footer}</footer>}
    </div>
  );
}

/** The intro video: in the show it loops silently from the start; elsewhere it stands on its first frame, or plays muted in a preview. */
function IntroVideo({ src, mode }: { src: string; mode: ScreenMode }) {
  // A missing file shows nothing rather than a broken video on the projector.
  const [failed, setFailed] = useState(false);
  if (failed) return null;
  const show = mode === "show";
  return <video src={src} autoPlay={show} loop muted playsInline controls={mode === "preview"} preload={show ? "auto" : "metadata"} onError={() => setFailed(true)} />;
}
