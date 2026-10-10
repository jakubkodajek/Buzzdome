import { ask, open } from "@tauri-apps/plugin-dialog";
import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { api } from "../api";
import { useApp, useReport, useRun } from "../app-context";
import type { SlideSound } from "../audio/slide-sound";
import { MAX_DROPPED_MB } from "../bindings/Constants";
import { ContextMenu } from "../editor/controls";
import { trackTime } from "../format";
import { AddIcon, GridIcon, ListIcon, PauseIcon, PlayIcon, RewindIcon, StopIcon, TrashIcon } from "../icons";
import { mediaFileFilter, mediaFileName } from "../media";
import { progressOf, searchFiles } from "../media-player";
import { readPreference, writePreference } from "../preferences";
import type { GameView, LibraryFile, MediaKind } from "../types";
import { carriesFiles, KindIcon, LIBRARY_FILE_TYPE, useTicking } from "./media-parts";
import { SoundBoardGrid } from "./SoundBoardGrid";
import type { MediaPlayer } from "./useMediaPlayer";

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
  const { t, notify } = useApp();
  const run = useRun();
  const report = useReport();
  const { library, now } = player;
  const [selected, setSelected] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [layout, setLayout] = useState(storedLayout);
  const [split, setSplit] = useState(storedSplit);
  const [menu, setMenu] = useState<{ file: LibraryFile; x: number; y: number } | null>(null);
  const [dropping, setDropping] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const ticking = useTicking(now?.sinceMs !== null && now !== null);

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

  const add = async () => {
    const paths = await open({ multiple: true, filters: [mediaFileFilter(t, ALL_KINDS)] });
    if (!paths || paths.length === 0) return;
    const added = await api.mediaAdd(Array.isArray(paths) ? paths : [paths]);
    if (added[0]) setSelected(added[0]);
  };

  const remove = async (reference: string) => {
    if (await ask(t("mediaRemoveConfirm", { name: mediaFileName(reference) }), { kind: "warning" })) await api.mediaRemove(reference);
  };

  /** Copies files dropped from the system into the library; returns the references of those that came in. */
  const importDropped = async (dropped: File[]): Promise<string[]> => {
    const references: string[] = [];
    for (const file of dropped) {
      if (file.size > MAX_DROPPED_MB * 2 ** 20) {
        notify(t("mediaDropTooLarge", { name: file.name, mb: MAX_DROPPED_MB }));
        continue;
      }
      try {
        references.push(await api.mediaAddDropped(file));
      } catch (error) {
        report(error);
      }
    }
    return references;
  };

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

  const playing = now !== null && now.sinceMs !== null;
  const showList = layout !== "board";
  const showBoard = layout !== "list";

  return (
    <div className="media-player">
      <div className="media-toolbar" role="toolbar">
        <button title={t("mediaAddHint")} onClick={() => run(add())}>
          <AddIcon /> {t("mediaAdd")}
        </button>
        <button disabled={!chosen} onClick={() => chosen && run(remove(chosen))}>
          <TrashIcon /> {t("mediaRemove")}
        </button>
        <span className="toolbar-gap" />
        <button disabled={!chosen} onClick={() => chosen && play(chosen)}>
          <PlayIcon /> {t("mediaPlay")}
        </button>
        <button disabled={!playing} onClick={() => control({ type: "pause" })}>
          <PauseIcon /> {t("pause")}
        </button>
        <button disabled={!now} onClick={() => control({ type: "stop" })}>
          <StopIcon /> {t("mediaStop")}
        </button>
        <button disabled={!now} onClick={() => control({ type: "rewind" })}>
          <RewindIcon /> {t("mediaRewind")}
        </button>
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
                if (e.key !== "Escape") return;
                e.preventDefault();
                setQuery("");
              }}
            />
            <ul className="media-files" role="listbox" aria-label={t("tabMedia")}>
              {found.map((file) => {
                const progress = progressOf(file.reference, now, player.durationOf(file.reference), ticking);
                const length = player.durationOf(file.reference);
                return (
                  <li
                    key={file.reference}
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
