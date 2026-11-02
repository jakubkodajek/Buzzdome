import { useApp } from "./app-context";
import { fileName } from "./format";
import { CrossIcon } from "./icons";
import "./recent-files.css";

/**
 * The recent quiz files as buttons, each its file name over its path, and
 * beside each a button that takes it off the list. `onOpen` and `onForget`
 * get the path clicked.
 */
export function RecentFileList(props: { files: string[]; disabled?: boolean; onOpen: (path: string) => void; onForget: (path: string) => void }) {
  const { files, disabled, onOpen, onForget } = props;
  const { t } = useApp();
  return (
    <ul className="recent-list">
      {files.map((path) => (
        <li key={path}>
          <button type="button" className="recent-open" disabled={disabled} onClick={() => onOpen(path)}>
            <b>{fileName(path)}</b>
            <small>{path}</small>
          </button>
          <button
            type="button"
            className="recent-forget"
            title={t("forgetRecentFile")}
            aria-label={`${t("forgetRecentFile")}: ${fileName(path)}`}
            disabled={disabled}
            onClick={() => onForget(path)}
          >
            <CrossIcon />
          </button>
        </li>
      ))}
    </ul>
  );
}
