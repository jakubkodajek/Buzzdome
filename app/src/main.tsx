import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { installBrowserGuard } from "./browser-guard";
import "./styles.css";

installBrowserGuard(window);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
