// The built-in themes as `list_themes` returns them, for the browser preview
// and the UI tests; a test in src-tauri/src/themes.rs writes them. JSON
// types the named values of a theme (`corners`, `headings`) as any string,
// so they are typed here once: Rust wrote them from their types.
import themes from "../bindings/BuiltinThemes.json";
import type { Theme } from "../types";

export const BUILTIN_THEMES = themes as readonly Theme[];
