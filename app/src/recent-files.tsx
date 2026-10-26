import { fileName } from "./format";
import "./recent-files.css";

/** The recent quiz files as buttons, each its file name over its path; `onOpen` gets the path clicked. */
export function RecentFileList({ files, disabled, onOpen }: { files: string[]; disabled?: boolean; onOpen: (path: string) => void }) {
  return (
    <ul className="recent-list">
      {files.map((path) => (
        <li key={path}>
          <button type="button" disabled={disabled} onClick={() => onOpen(path)}>
            <b>{fileName(path)}</b>
            <small>{path}</small>
          </button>
        </li>
      ))}
    </ul>
  );
}
