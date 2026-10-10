import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri", "src/bindings"] },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [js.configs.recommended, tseslint.configs.recommended, reactHooks.configs.flat.recommended],
    languageOptions: { globals: globals.browser },
    rules: {
      // A missing dependency is a stale-state bug in a live show, not a style issue.
      "react-hooks/exhaustive-deps": "error",
      // The web view's own dialogs block it and may not show at all; use confirmAction (confirm.ts).
      "no-restricted-globals": ["error", "alert", "confirm", "prompt"],
      // The backend picks files (A21), and confirm.ts asks.
      "no-restricted-imports": [
        "error",
        { paths: [{ name: "@tauri-apps/plugin-dialog", message: "Ask with confirmAction (confirm.ts); the backend opens file pickers (dialogs.rs)." }] },
      ],
    },
  },
  {
    files: ["src/confirm.ts"],
    rules: { "no-restricted-imports": "off" },
  },
  {
    // The windows count on the show clock from what they heard, by one clock (media-player.ts).
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/media-player.ts", "src/dev/**", "src/**/*.test.ts"],
    rules: {
      "no-restricted-properties": [
        "error",
        { object: "performance", property: "now", message: "Count on the show clock with showClockNow or heardNow (media-player.ts)." },
        { object: "Date", property: "now", message: "The show clock is the backend's; count on it with showClockNow (media-player.ts)." },
      ],
    },
  },
);
