import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { api } from "./api";
import { installBrowserGuard } from "./browser-guard";
import { installBlockNotes } from "./csp-report";
import "./styles.css";

installBrowserGuard(window);
installBlockNotes(window, (line) => void api.logProblem(line).catch(() => undefined));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
