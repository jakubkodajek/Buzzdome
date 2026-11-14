// Team names typed or pasted into the Teams tab. The core cleans them up
// (spaces, repeats, length; `Command::SetTeams`), so this only splits them;
// a file of names is read in Rust (`api.addTeamFile`).

/** The largest file of team names read: 200 names fit in a few kilobytes (`MAX_TEAMS`). */
export const MAX_TEAM_FILE_KB = 1024;

/** The names in `text`, one per line; blank lines give no name. */
export function parseTeamNames(text: string): string[] {
  return text
    .replace(/^\uFEFF/, "")
    .split(/\r\n|\r|\n/)
    .map((line) => line.trim())
    .filter((name) => name !== "");
}
