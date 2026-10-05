import { getCurrentWindow } from "@tauri-apps/api/window";
import { Director } from "./director/Director";
import { HostScreen } from "./host/HostScreen";

// Both windows load the same bundle; the window label picks the screen.
const isHost = getCurrentWindow().label === "host";

export default function App() {
  return isHost ? <HostScreen /> : <Director />;
}
