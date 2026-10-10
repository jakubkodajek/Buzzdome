import { useState, type DragEvent } from "react";
import { api } from "../api";
import { useApp, useDispatch, useRun } from "../app-context";
import type { SlideSound } from "../audio/slide-sound";
import { MAX_BOARD_COLUMNS, MAX_BOARD_ROWS, MAX_PAD_LABEL_CHARS, MIN_BOARD_COLUMNS, MIN_BOARD_ROWS } from "../bindings/Constants";
import { ContextMenu, Dialog, Popup } from "../editor/controls";
import type { Action } from "../editor/actions";
import { trackTime } from "../format";
import type { StringKey } from "../i18n";
import { GridIcon, PauseIcon, PlayIcon, SoundIcon, StopIcon, TrashIcon, VideoIcon } from "../icons";
import { mediaFileName } from "../media";
import { PAD_COLORS, padColor, padFile, progressOf, shownRows, type Progress } from "../media-player";
import type { BoardChange, GameView, MediaKind, Pad, PadClick, PadColor, PadSource } from "../types";
import { carriesFiles, KindIcon, LIBRARY_FILE_TYPE } from "./media-parts";
import type { MediaPlayer } from "./useMediaPlayer";
import { confirmAction } from "../confirm";

/** Drag data of a pad: its place. */
const PAD_TYPE = "application/x-quiz-show-pad";

const COLOR_KEYS: Record<PadColor, StringKey> = {
  red: "colorRed",
  orange: "colorOrange",
  yellow: "colorYellow",
  green: "colorGreen",
  teal: "colorTeal",
  blue: "colorBlue",
  purple: "colorPurple",
  pink: "colorPink",
};

const CLICK_KEYS: Record<PadClick, StringKey> = {
  stop: "boardClickStop",
  restart: "boardClickRestart",
  pause: "boardClickPause",
};

type Open =
  | { what: "padMenu" | "colors"; place: number; x: number; y: number }
  | { what: "boardMenu" | "gridSize"; x: number; y: number }
  | { what: "label"; place: number }
  | null;

interface SoundBoardGridProps {
  player: MediaPlayer;
  sound: SlideSound;
  view: GameView | null;
  /** The file selected in the list, which the pad menu can put on a pad. */
  selected: string | null;
  onSelect: (reference: string) => void;
  importDropped: (files: File[]) => Promise<string[]>;
  /** The time now, renewed while something plays. */
  ticking: number;
}

/**
 * The sound board (spec SN-7): a grid of pads that play a library file, or
 * the slide's sound or video, with one click. A playing pad is lit and
 * counts down; what a click on it does is set in the board's menu. Files
 * are dragged onto pads from the list or from a folder, and pads onto pads
 * to swap them; the right mouse button sets up a pad or the board.
 */
export function SoundBoardGrid({ player, sound, view, selected, onSelect, importDropped, ticking }: SoundBoardGridProps) {
  const { t } = useApp();
  const run = useRun();
  const dispatch = useDispatch();
  const { board, now, library } = player;
  const [open, setOpen] = useState<Open>(null);
  const [target, setTarget] = useState<number | null>(null);
  if (!board || !library) return <section className="sound-board" />;

  const change = (next: BoardChange) => run(api.soundBoardChange(next));
  const kindOf = (reference: string): MediaKind | null => library.files.find((file) => file.reference === reference)?.kind ?? null;
  const slideVideoReady = view?.controls.playVideo ?? false;

  const progressOfPad = (pad: Pad): Progress => {
    const file = padFile(pad);
    if (file) return progressOf(file, now, player.durationOf(file), ticking);
    return pad.source?.type === "slideSound" && sound.playing ? { state: "playing", remainingMs: null, done: null } : { state: "idle" };
  };
  const ready = (source: PadSource | null) =>
    source !== null && (source.type === "file" || (source.type === "slideSound" ? sound.available : slideVideoReady));

  const click = (place: number, pad: Pad) => {
    switch (pad.source?.type) {
      case "file":
        onSelect(pad.source.reference);
        run(api.mediaControl({ type: "pad", pad: place }));
        break;
      case "slideSound":
        if (sound.playing) sound.stop();
        else if (sound.available) sound.play();
        break;
      case "slideVideo":
        if (slideVideoReady) dispatch({ type: "playVideo" });
        break;
    }
  };

  const drop = async (place: number, e: DragEvent) => {
    const swapped = e.dataTransfer.getData(PAD_TYPE);
    if (swapped !== "") return Number(swapped) !== place && change({ type: "swap", a: Number(swapped), b: place });
    const reference = e.dataTransfer.getData(LIBRARY_FILE_TYPE);
    const references = reference !== "" ? [reference] : await importDropped([...e.dataTransfer.files]);
    if (references.length > 0) change({ type: "place", pad: place, references });
  };

  const accepts = (e: DragEvent) => e.dataTransfer.types.includes(PAD_TYPE) || e.dataTransfer.types.includes(LIBRARY_FILE_TYPE) || carriesFiles(e);

  const padMenu = (place: number, x: number, y: number): (Action | "separator")[] => {
    const pad = board.pads[place];
    const source = pad.source;
    const lit = progressOfPad(pad).state === "playing";
    const setSource = (next: PadSource | null) => change({ type: "setSource", pad: place, source: next });
    return [
      { label: t(lit ? "mediaStop" : "mediaPlay"), icon: lit ? <StopIcon /> : <PlayIcon />, disabled: !ready(source), run: () => click(place, pad) },
      "separator",
      {
        label: selected ? t("padFile", { name: mediaFileName(selected) }) : t("padFileNone"),
        disabled: !selected,
        checked: selected !== null && padFile(pad) === selected ? true : undefined,
        run: () => selected && setSource({ type: "file", reference: selected }),
      },
      { label: t("padSlideSound"), icon: <SoundIcon />, checked: source?.type === "slideSound" || undefined, run: () => setSource({ type: "slideSound" }) },
      { label: t("padSlideVideo"), icon: <VideoIcon />, checked: source?.type === "slideVideo" || undefined, run: () => setSource({ type: "slideVideo" }) },
      "separator",
      { label: t("padLabel"), run: () => setOpen({ what: "label", place }) },
      { label: t("padColor"), run: () => setOpen({ what: "colors", place, x, y }) },
      "separator",
      {
        label: t("padClear"),
        icon: <TrashIcon />,
        disabled: source === null && pad.label === "" && pad.color === null,
        run: () => change({ type: "clearPad", pad: place }),
      },
    ];
  };

  const boardMenu = (x: number, y: number): (Action | "separator")[] => [
    { label: t("boardGridSize"), icon: <GridIcon />, run: () => setOpen({ what: "gridSize", x, y }) },
    "separator",
    ...(Object.keys(CLICK_KEYS) as PadClick[]).map((choice) => ({
      label: t(CLICK_KEYS[choice]),
      checked: board.click === choice,
      run: () => change({ type: "setClick", click: choice }),
    })),
    "separator",
    { label: t("boardFill"), run: () => change({ type: "fillEmpty" }) },
    {
      label: t("boardClear"),
      icon: <TrashIcon />,
      run: () => run(confirmAction(t, t("boardClearConfirm")).then((yes) => (yes ? api.soundBoardChange({ type: "clearAll" }) : undefined))),
    },
  ];

  const labelOf = (pad: Pad): string => {
    if (pad.label) return pad.label;
    switch (pad.source?.type) {
      case "file":
        return mediaFileName(pad.source.reference);
      case "slideSound":
        return t("padSlideSound");
      case "slideVideo":
        return t("padSlideVideo");
      default:
        return "";
    }
  };

  return (
    <section
      className="sound-board"
      onContextMenu={(e) => {
        e.preventDefault();
        setOpen({ what: "boardMenu", x: e.clientX, y: e.clientY });
      }}
    >
      <header className="board-header">
        <button
          onClick={(e) => {
            const box = e.currentTarget.getBoundingClientRect();
            setOpen({ what: "boardMenu", x: box.left, y: box.bottom });
          }}
        >
          <GridIcon /> {t("boardOptions")} <span className="muted">{t("boardGrid", { rows: board.rows, columns: board.columns })}</span>
        </button>
        <span className="muted board-hint">{t("boardHint")}</span>
      </header>
      <div className="board-grid" style={{ gridTemplateColumns: `repeat(${board.columns}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${board.rows}, minmax(0, 1fr))` }}>
        {shownRows(board).flatMap((places) =>
          places.map((place) => {
            const pad = board.pads[place];
            const progress = progressOfPad(pad);
            const file = padFile(pad);
            const kind = file ? kindOf(file) : null;
            const length = file ? player.durationOf(file) : undefined;
            return (
              <button
                key={place}
                className={["pad", `pad-${padColor(pad, place)}`, pad.source ? "" : "empty", progress.state, ready(pad.source) ? "" : "unready", target === place ? "target" : ""].join(" ")}
                aria-pressed={progress.state === "playing"}
                draggable={pad.source !== null}
                onClick={() => click(place, pad)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setOpen({ what: "padMenu", place, x: e.clientX, y: e.clientY });
                }}
                onDragStart={(e) => {
                  e.dataTransfer.setData(PAD_TYPE, String(place));
                  e.dataTransfer.effectAllowed = "move";
                }}
                onDragOver={(e) => {
                  if (!accepts(e)) return;
                  e.preventDefault();
                  e.dataTransfer.dropEffect = e.dataTransfer.types.includes(PAD_TYPE) ? "move" : "copy";
                  setTarget(place);
                }}
                onDragLeave={() => setTarget((current) => (current === place ? null : current))}
                onDrop={(e) => {
                  if (!accepts(e)) return;
                  e.preventDefault();
                  setTarget(null);
                  run(drop(place, e));
                }}
              >
                <span className="pad-label">{labelOf(pad)}</span>
                <span className="pad-foot">
                  <span className="pad-kind">
                    {kind ? <KindIcon kind={kind} /> : pad.source?.type === "slideSound" ? <SoundIcon /> : pad.source?.type === "slideVideo" ? <VideoIcon /> : null}
                  </span>
                  <span className="pad-time tabular">
                    {progress.state === "paused" && <PauseIcon />}
                    {progress.state !== "idle" && progress.remainingMs !== null ? trackTime(progress.remainingMs) : length && length > 0 ? trackTime(length) : ""}
                  </span>
                </span>
                {progress.state !== "idle" && progress.done !== null && <span className="pad-progress" style={{ width: `${progress.done * 100}%` }} />}
              </button>
            );
          }),
        )}
      </div>
      {open?.what === "padMenu" && <ContextMenu x={open.x} y={open.y} items={padMenu(open.place, open.x, open.y)} onClose={() => setOpen((current) => (current?.what === "padMenu" ? null : current))} />}
      {open?.what === "boardMenu" && (
        <ContextMenu x={open.x} y={open.y} items={boardMenu(open.x, open.y)} onClose={() => setOpen((current) => (current?.what === "boardMenu" ? null : current))} />
      )}
      {open?.what === "colors" && (
        <ColorPicker
          x={open.x}
          y={open.y}
          color={board.pads[open.place].color}
          onPick={(color) => change({ type: "setColor", pad: open.place, color })}
          onClose={() => setOpen(null)}
        />
      )}
      {open?.what === "gridSize" && (
        <GridSizePicker x={open.x} y={open.y} rows={board.rows} columns={board.columns} onPick={(rows, columns) => change({ type: "resize", rows, columns })} onClose={() => setOpen(null)} />
      )}
      {open?.what === "label" && (
        <LabelDialog label={board.pads[open.place].label} onApply={(label) => change({ type: "setLabel", pad: open.place, label })} onClose={() => setOpen(null)} />
      )}
    </section>
  );
}

function ColorPicker(props: { x: number; y: number; color: PadColor | null; onPick: (color: PadColor | null) => void; onClose: () => void }) {
  const { x, y, color, onPick, onClose } = props;
  const { t } = useApp();
  const pick = (next: PadColor | null) => {
    onPick(next);
    onClose();
  };
  return (
    <Popup x={x} y={y} onClose={onClose} className="pad-colors" role="menu" label={t("padColor")}>
      <button type="button" role="menuitemradio" aria-checked={color === null} className="pad-color-auto" onClick={() => pick(null)}>
        {t("padColorAuto")}
      </button>
      <div className="pad-swatches">
        {PAD_COLORS.map((choice) => (
          <button
            key={choice}
            type="button"
            role="menuitemradio"
            aria-checked={color === choice}
            className={`pad-swatch pad-${choice}`}
            title={t(COLOR_KEYS[choice])}
            aria-label={t(COLOR_KEYS[choice])}
            onClick={() => pick(choice)}
          />
        ))}
      </div>
    </Popup>
  );
}

/** Picks the grid's size by pointing at its lower right pad, as in a table picker. */
function GridSizePicker(props: { x: number; y: number; rows: number; columns: number; onPick: (rows: number, columns: number) => void; onClose: () => void }) {
  const { x, y, onPick, onClose } = props;
  const { t } = useApp();
  const [size, setSize] = useState({ rows: props.rows, columns: props.columns });
  const point = (row: number, column: number) => setSize({ rows: Math.max(MIN_BOARD_ROWS, row), columns: Math.max(MIN_BOARD_COLUMNS, column) });
  return (
    <Popup x={x} y={y} onClose={onClose} className="grid-size" role="dialog" label={t("boardGridSize")}>
      <div className="grid-size-cells" style={{ gridTemplateColumns: `repeat(${MAX_BOARD_COLUMNS}, 18px)` }}>
        {Array.from({ length: MAX_BOARD_ROWS }, (_, r) =>
          Array.from({ length: MAX_BOARD_COLUMNS }, (_, c) => (
            <button
              key={`${r}-${c}`}
              type="button"
              aria-label={t("boardGrid", { rows: Math.max(MIN_BOARD_ROWS, r + 1), columns: Math.max(MIN_BOARD_COLUMNS, c + 1) })}
              className={r < size.rows && c < size.columns ? "on" : ""}
              onMouseEnter={() => point(r + 1, c + 1)}
              onFocus={() => point(r + 1, c + 1)}
              onClick={() => {
                onPick(size.rows, size.columns);
                onClose();
              }}
            />
          )),
        )}
      </div>
      <div className="grid-size-label">{t("boardGrid", size)}</div>
    </Popup>
  );
}

function LabelDialog({ label, onApply, onClose }: { label: string; onApply: (label: string) => void; onClose: () => void }) {
  const { t } = useApp();
  const [text, setText] = useState(label);
  const apply = () => {
    onApply(text);
    onClose();
  };
  return (
    <Dialog
      title={t("padLabelTitle")}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="primary" onClick={apply}>
            {t("ok")}
          </button>
          <button type="button" onClick={onClose}>
            {t("cancel")}
          </button>
        </>
      }
    >
      <input value={text} maxLength={MAX_PAD_LABEL_CHARS} aria-label={t("padLabelTitle")} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && apply()} />
      <p className="muted">{t("padLabelHint")}</p>
    </Dialog>
  );
}
