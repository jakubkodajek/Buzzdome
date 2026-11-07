import { useId, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { api } from "../api";
import { useApp, useReport, useRun } from "../app-context";
import type { SlideSound } from "../audio/slide-sound";
import { MAX_DROPPED_MB, MAX_PLAYER_VOLUME } from "../bindings/Constants";
import { ContextMenu } from "../editor/controls";
import { trackTime } from "../format";
import { usePlayerVolume } from "../hooks";
import { AddIcon, GridIcon, ListIcon, PauseIcon, PlayIcon, RewindIcon, SoundIcon, StopIcon, TrashIcon } from "../icons";
import { mediaFileName, mediaPicker } from "../media";
import { settingsLostNotice } from "../messages";
import { NoticeBox } from "../notice";
import { padsWithFile, progressOf, searchFiles, stepSelection } from "../media-player";
import { readPreference, writePreference } from "../preferences";
import type { GameView, LibraryFile, MediaKind } from "../types";
import { carriesFiles, KindIcon, LIBRARY_FILE_TYPE, useTicking } from "./media-parts";
import { SoundBoardGrid } from "./SoundBoardGrid";
import type { MediaPlayer } from "./useMediaPlayer";
import { confirmAction } from "../confirm";

/** Dropped files are sent whole through memory; bigger ones are added with Add, from their path. */
const LAYOUT_KEY = "quiz-show.media-player.layout";
const SPLIT_KEY = "quiz-show.media-player.split";
/** The list's share of the width when both are shown, and its limits. */
const DEFAULT_SPLIT = 0.4;
const [MIN_SPLIT, MAX_SPLIT] = [0.2, 0.75];
const ALL_KINDS: MediaKind[] = ["image", "audio", "video"];

type Layout = "both" | "list" | "board";

function storedLayout(): Layout {
  const stored = readPreference(LAYOUT_KEY);
  return stored === "list" || stored === "board" ? stored : "both";
}

function storedSplit(): number {
  const stored = Number(readPreference(SPLIT_KEY));
  return stored >= MIN_SPLIT && stored <= MAX_SPLIT ? stored : DEFAULT_SPLIT;
}

/**
 * The Director's media player (spec SN-7): a library of pictures, sounds and
 * videos kept on this computer, with a search, and the sound board. Sounds
 * play in the Director, pictures and videos on the projector. The toolbar
 * acts on the file selected in the list, which a click on a pad selects too.
 */
export function MediaPlayerTab({ player, sound, view }: { player: MediaPlayer; sound: SlideSound; view: GameView | null }) {
  const { t, caution, notify } = useApp();
  const run = useRun();
  const report = useReport();
  const { library, now } = player;
  const [selected, setSelected] = useState<string | null>(null);
  // Copies into the library under way, from Add or a drop on the list or a pad.
  const [copying, setCopying] = useState(0);
  const list = useRef<HTMLUListElement>(null);
  const listId = useId();
  const [query, setQuery] = useState("");
  const [layout, setLayout] = useState(storedLayout);
  const [split, setSplit] = useState(storedSplit);
  const [menu, setMenu] = useState<{ file: LibraryFile; x: number; y: number } | null>(null);
  const [dropping, setDropping] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const ticking = useTicking(player.heard);

  const files = library?.files ?? [];
  const found = searchFiles(files, query);
  const chosen = selected !== null && files.some((file) => file.reference === selected) ? selected : null;

  const control = (command: Parameters<typeof api.mediaControl>[0]) => run(api.mediaControl(command));
  const play = (reference: string) => control({ type: "play", reference });
  const changeLayout = (next: Layout) => {
    setLayout(next);
    writePreference(LAYOUT_KEY, next);
  };
  /** Shows or hides one of the two; one always stays. */
  const toggle = (part: "list" | "board") => {
    const other = part === "list" ? "board" : "list";
    if (layout === "both") changeLayout(other);
    else if (layout === other) changeLayout("both");
  };

  /** Runs `copy` into the library, with Add off and the toolbar saying so meanwhile, and says how many files came in. */
  const copyIn = async (copy: () => Promise<string[]>): Promise<string[]> => {
    setCopying((n) => n + 1);
    try {
      const added = await copy();
      if (added.length > 0) notify(t("mediaAdded", { n: added.length }));
      return added;
    } finally {
      setCopying((n) => n - 1);
    }
  };

  const add = async () => {
    const added = await copyIn(() => api.mediaAdd(mediaPicker(t, ALL_KINDS)));
    if (added[0]) setSelected(added[0]);
  };

  // Removing a file stops it if it plays and empties the pads that hold it (quiz-core `library_changed`).
  const remove = async (reference: string) => {
    const pads = player.board ? padsWithFile(player.board, reference) : 0;
    const message = [
      t("mediaRemoveConfirm", { name: mediaFileName(reference) }),
      now?.reference === reference ? t("mediaRemoveStops") : "",
      pads > 0 ? t("mediaRemoveEmptiesPads", { n: pads }) : "",
    ];
    if (await confirmAction(t, message.filter((part) => part !== "").join(" "))) await api.mediaRemove(reference);
  };

  /** Copies files dropped from the system into the library; returns the references of those that came in. */
  const importDropped = (dropped: File[]): Promise<string[]> =>
    copyIn(async () => {
      const references: string[] = [];
      for (const file of dropped) {
        if (file.size > MAX_DROPPED_MB * 2 ** 20) {
          caution(t("mediaDropTooLarge", { name: file.name, mb: MAX_DROPPED_MB }));
          continue;
        }
        try {
          references.push(await api.mediaAddDropped(file));
        } catch (error) {
          report(error);
        }
      }
      return references;
    });

  const startDivider = (e: ReactPointerEvent<HTMLDivElement>) => {
    const box = body.current?.getBoundingClientRect();
    if (!box) return;
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const move = (event: PointerEvent) => setSplit(Math.min(MAX_SPLIT, Math.max(MIN_SPLIT, (event.clientX - box.left) / box.width)));
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      setSplit((value) => {
        writePreference(SPLIT_KEY, String(value));
        return value;
      });
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
  };

  const paused = now !== null && now.sinceMs === null;
  const showList = layout !== "board";
  const showBoard = layout !== "list";

  // The keys work on the files the search shows.
  const references = found.map((file) => file.reference);
  const listed = chosen !== null && references.includes(chosen) ? chosen : null;
  const optionId = (reference: string) => `${listId}-${references.indexOf(reference)}`;
  const onListKey = (e: ReactKeyboardEvent<HTMLUListElement>) => {
    if ((e.key === "Enter" || e.key === "Delete") && listed) {
      e.preventDefault();
      if (e.key === "Enter") play(listed);
      else run(remove(listed));
      return;
    }
    const next = stepSelection(references, listed, e.key);
    if (next === null) return;
    e.preventDefault();
    setSelected(next);
    document.getElementById(optionId(next))?.scrollIntoView({ block: "nearest" });
  };

  return (
    <div className="media-player">
      <div className="media-toolbar" role="toolbar">
        <button title={t("mediaAddHint")} disabled={copying > 0} onClick={() => run(add())}>
          <AddIcon /> {t("mediaAdd")}
        </button>
        <button disabled={!chosen} onClick={() => chosen && run(remove(chosen))}>
          <TrashIcon /> {t("mediaRemove")}
        </button>
        {copying > 0 && (
          <span className="muted" role="status">
            {t("mediaCopying")}
          </span>
        )}
        <span className="toolbar-gap" />
        <button disabled={!chosen} onClick={() => chosen && play(chosen)}>
          <PlayIcon /> {t("mediaPlay")}
        </button>
        {/* Resumes the paused file, whichever file is selected meanwhile. */}
        <button disabled={!now} onClick={() => control({ type: paused ? "resume" : "pause" })}>
          {paused ? <PlayIcon /> : <PauseIcon />} {t(paused ? "mediaResume" : "pause")}
        </button>
        <button disabled={!now} onClick={() => control({ type: "stop" })}>
          <StopIcon /> {t("mediaStop")}
        </button>
        <button disabled={!now} onClick={() => control({ type: "rewind" })}>
          <RewindIcon /> {t("mediaRewind")}
        </button>
        <PlayerVolume />
        {now && (
          <span className="media-now">
            <KindIcon kind={now.kind} /> {mediaFileName(now.reference)}
          </span>
        )}
        <span className="spacer" />
        <button aria-pressed={showList} className={showList ? "toggle on" : "toggle"} onClick={() => toggle("list")}>
          <ListIcon /> {t("mediaShowList")}
        </button>
        <button aria-pressed={showBoard} className={showBoard ? "toggle on" : "toggle"} onClick={() => toggle("board")}>
          <GridIcon /> {t("mediaShowBoard")}
        </button>
      </div>
      {library?.settingsError && <NoticeBox notice={settingsLostNotice(t, t("mediaSettingsLost"), library.settingsError)} />}
      <div
        ref={body}
        className="media-body"
        style={{ gridTemplateColumns: layout === "both" ? `${split * 100}% 6px minmax(0, 1fr)` : "minmax(0, 1fr)" }}
      >
        {showList && (
          <section
            className={dropping ? "media-list dropping" : "media-list"}
            onDragOver={(e) => {
              if (!carriesFiles(e)) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = "copy";
              setDropping(true);
            }}
            onDragLeave={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && setDropping(false)}
            onDrop={(e) => {
              if (!carriesFiles(e)) return;
              e.preventDefault();
              setDropping(false);
              const dropped = [...e.dataTransfer.files];
              run(importDropped(dropped).then((references) => references[0] && setSelected(references[0])));
            }}
          >
            <input
              type="search"
              className="media-search"
              value={query}
              placeholder={t("mediaSearch")}
              aria-label={t("mediaSearch")}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  setQuery("");
                } else if (e.key === "ArrowDown" && references.length > 0) {
                  e.preventDefault();
                  if (!listed) setSelected(references[0]);
                  list.current?.focus();
                }
              }}
            />
            <ul
              ref={list}
              className="media-files"
              role="listbox"
              aria-label={t("tabMedia")}
              tabIndex={0}
              aria-activedescendant={listed ? optionId(listed) : undefined}
              onKeyDown={onListKey}
            >
              {found.map((file) => {
                const progress = progressOf(file.reference, now, player.durationOf(file.reference), ticking);
                const length = player.durationOf(file.reference);
                return (
                  <li
                    key={file.reference}
                    id={optionId(file.reference)}
                    role="option"
                    aria-selected={file.reference === chosen}
                    className={[file.reference === chosen ? "selected" : "", progress.state].join(" ")}
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData(LIBRARY_FILE_TYPE, file.reference);
                      e.dataTransfer.effectAllowed = "copy";
                    }}
                    onClick={() => setSelected(file.reference)}
                    onDoubleClick={() => play(file.reference)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setSelected(file.reference);
                      setMenu({ file, x: e.clientX, y: e.clientY });
                    }}
                  >
                    <span className="library-kind">
                      {progress.state === "playing" ? <PlayIcon /> : progress.state === "paused" ? <PauseIcon /> : <KindIcon kind={file.kind} />}
                    </span>
                    <span className="library-name">{mediaFileName(file.reference)}</span>
                    <span className="library-length tabular">
                      {progress.state !== "idle" && progress.remainingMs !== null ? `-${trackTime(progress.remainingMs)}` : length && length > 0 ? trackTime(length) : ""}
                    </span>
                  </li>
                );
              })}
            </ul>
            {files.length === 0 ? <p className="muted media-empty">{t("mediaEmpty")}</p> : found.length === 0 && <p className="muted media-empty">{t("mediaNoMatch")}</p>}
          </section>
        )}
        {layout === "both" && <div className="media-divider" role="separator" aria-orientation="vertical" title={t("mediaDivider")} onPointerDown={startDivider} />}
        {showBoard && <SoundBoardGrid player={player} sound={sound} view={view} selected={chosen} onSelect={setSelected} importDropped={importDropped} ticking={ticking} />}
      </div>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: t("mediaPlay"), icon: <PlayIcon />, run: () => play(menu.file.reference) },
            "separator",
            { label: t("mediaRemove"), icon: <TrashIcon />, run: () => run(remove(menu.file.reference)) },
          ]}
        />
      )}
    </div>
  );
}

/** The player's volume, which everything the show plays follows at once. */
function PlayerVolume() {
  const { t } = useApp();
  const run = useRun();
  const volume = usePlayerVolume();
  return (
    <label className="media-volume" title={t("playerVolumeHint")}>
      <SoundIcon />
      <input
        type="range"
        min={0}
        max={MAX_PLAYER_VOLUME}
        value={volume}
        aria-label={t("playerVolume")}
        onChange={(e) => run(api.mediaSetVolume(Number(e.target.value)))}
      />
      <span className="media-volume-value">{t("percent", { n: volume })}</span>
    </label>
  );
}
