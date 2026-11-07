import { useState, type ReactNode } from "react";
import { useApp } from "../app-context";
import { mediaFileName } from "../media";
import type { ImportNote, Quiz, Slide } from "../types";
import type { Action } from "./actions";
import { ImportNotesDialog } from "./ImportNotesDialog";
import { templateQuiz } from "./sheet";
import type { QuizEditor } from "./useQuizEditor";

export interface SheetActions {
  importSheet: Action;
  sheetTemplate: Action;
  exportSheet: Action;
  /** Whether the import notes are shown, which then take the keys. */
  windowOpen: boolean;
  windows: ReactNode;
}

/**
 * File › Import and Export with spreadsheets (spec QZ-8). An import hands
 * the sheet's slides to `insertImported` once its dialogs are closed, which
 * inserts them where the editor is then and says how many it inserted; what
 * the sheet had that could not be taken over is listed.
 */
export function useSheetActions(editor: QuizEditor, quiz: Quiz | null, insertImported: (slides: Slide[]) => number): SheetActions {
  const { t, notify, caution } = useApp();
  const [importNotes, setImportNotes] = useState<{ imported: number; notes: ImportNote[] } | null>(null);
  // Not while the editor works on a file.
  const off = !quiz || editor.status.busy !== null;
  return {
    importSheet: {
      label: t("importFromSheet"),
      disabled: off,
      run: () =>
        void editor.importSheet().then((result) => {
          if (!result) return;
          const imported = insertImported(result.slides);
          if (result.notes.length > 0) setImportNotes({ imported, notes: result.notes });
          else if (imported > 0) notify(t("sheetImported", { n: imported }));
          else caution(t("sheetImportedNone"));
        }),
    },
    sheetTemplate: {
      label: t("saveSheetTemplate"),
      disabled: off,
      run: () => quiz && void editor.exportSheet(templateQuiz(t, quiz), t("templateFileName")).then((done) => done && notify(t("sheetTemplateSaved"))),
    },
    exportSheet: {
      label: t("exportToSheet"),
      disabled: off,
      run: () =>
        quiz &&
        void editor.exportSheet(quiz, quiz.title || t("untitledQuiz")).then((report) => {
          if (!report) return;
          const missing = report.missing.map(mediaFileName).join(", ");
          const refused = report.refused.join(", ");
          const text = [
            t("sheetExported", { n: quiz.slides.length, media: report.media }),
            missing && t("sheetExportMissing", { files: missing }),
            refused && t("sheetExportRefused", { files: refused }),
          ]
            .filter(Boolean)
            .join(" ");
          // Done, but some media did not go along: worth a look.
          (missing || refused ? caution : notify)(text);
        }),
    },
    windowOpen: importNotes !== null,
    windows: importNotes && <ImportNotesDialog {...importNotes} onClose={() => setImportNotes(null)} />,
  };
}
