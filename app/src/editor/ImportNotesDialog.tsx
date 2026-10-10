import { useApp } from "../app-context";
import { importIssueText, placesText } from "../messages";
import type { ImportNote } from "../types";
import { Dialog } from "./controls";

/** What a spreadsheet import could not take over as the sheet has it, with the rows (spec QZ-8). */
export function ImportNotesDialog({ imported, notes, onClose }: { imported: number; notes: ImportNote[]; onClose: () => void }) {
  const { t } = useApp();
  return (
    <Dialog
      title={t("sheetImportNotes")}
      className="import-notes-dialog"
      onClose={onClose}
      footer={
        <button type="button" className="primary" data-autofocus onClick={onClose}>
          {t("ok")}
        </button>
      }
    >
      <p>{imported > 0 ? t("sheetImported", { n: imported }) : t("sheetImportedNone")}</p>
      <ul className="import-notes">
        {notes.map((note, i) => (
          <li key={i}>
            <span>{importIssueText(t, note.issue)}</span>
            <small>{placesText(t, note.places)}</small>
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
