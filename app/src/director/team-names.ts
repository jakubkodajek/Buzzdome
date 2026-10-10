// Team names brought in from pasted text or a file. The core cleans them up
// (spaces, repeats, length; `Command::SetTeams`), so this only splits them.

/**
 * Text from a file's bytes: UTF-8 when the bytes are valid UTF-8, otherwise
 * Windows-1250, the code page Czech Windows uses for files that a
 * spreadsheet saves as plain CSV.
 */
export function decodeText(bytes: ArrayBuffer): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder("windows-1250").decode(bytes);
  }
}

/**
 * The names in `text`: one per line, or in a CSV file (`csv`) the first
 * column, separated by `;` or `,` (whichever the first line has more of)
 * and possibly in double quotes. Blank lines give no name.
 */
export function parseTeamNames(text: string, csv: boolean): string[] {
  const lines = text.replace(/^\uFEFF/, "").split(/\r\n|\r|\n/);
  const separator = csv ? separatorOf(lines[0] ?? "") : null;
  return lines.map((line) => (separator ? firstField(line, separator) : line).trim()).filter((name) => name !== "");
}

function separatorOf(line: string): string {
  const count = (char: string) => line.split(char).length - 1;
  return count(";") >= count(",") ? ";" : ",";
}

/** The first field of a CSV line; `""` inside quotes is one `"`. */
function firstField(line: string, separator: string): string {
  if (!line.startsWith('"')) {
    const end = line.indexOf(separator);
    return end < 0 ? line : line.slice(0, end);
  }
  let field = "";
  for (let i = 1; i < line.length; i++) {
    if (line[i] !== '"') field += line[i];
    else if (line[i + 1] === '"') field += line[++i];
    else break;
  }
  return field;
}
