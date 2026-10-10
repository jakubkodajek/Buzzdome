import { useRef, useState, type DragEvent } from "react";
import { api } from "../api";
import { useApp, useDispatch, useRun } from "../app-context";
import { MAX_NAME_CHARS } from "../bindings/Constants";
import { CrossIcon, TrashIcon } from "../icons";
import type { BuzzerId, PlayerView, TeamView } from "../types";
import { decodeText, parseTeamNames } from "./team-names";

/** What a dragged buzzer number carries. */
const BUZZER_TYPE = "application/x-quiz-buzzer";

interface BuzzerChipProps {
  buzzer: BuzzerId;
  /** Not paired with a team yet. */
  free: boolean;
  /** Picked for pairing by a click; a click on a team name then pairs it. */
  picked: boolean;
  onPick: (buzzer: BuzzerId) => void;
}

/** A buzzer number to drag onto a team name, or to click before clicking the name. */
export function BuzzerChip({ buzzer, free, picked, onPick }: BuzzerChipProps) {
  const { t } = useApp();
  return (
    <span
      className={["buzzer-chip", free ? "free" : "", picked ? "picked" : ""].join(" ")}
      role="button"
      tabIndex={0}
      draggable
      aria-pressed={picked}
      title={t("pairHint")}
      onDragStart={(e) => {
        e.dataTransfer.setData(BUZZER_TYPE, String(buzzer));
        e.dataTransfer.effectAllowed = "link";
      }}
      onClick={(e) => {
        e.stopPropagation();
        onPick(buzzer);
      }}
      onKeyDown={(e) => {
        if (e.key !== "Enter" && e.key !== " ") return;
        e.preventDefault();
        onPick(buzzer);
      }}
    >
      {buzzer}
    </span>
  );
}

interface TeamListProps {
  teams: TeamView[];
  /** The show's players, whose buzzers pair with teams; null before a show. */
  players: PlayerView[] | null;
  picked: BuzzerId | null;
  /** Picks a buzzer for pairing, or ends that with null. */
  onPick: (buzzer: BuzzerId | null) => void;
}

/**
 * The team names prepared for shows, kept between them, and in a show their
 * buzzers. A buzzer is paired with a team by dragging its number onto the
 * name, or by clicking the number and then the name; the buzzer's player
 * then has the team's name (`pairTeam`).
 */
export function TeamList({ teams, players, picked, onPick }: TeamListProps) {
  const { t } = useApp();
  const dispatch = useDispatch();
  const run = useRun();
  const [draft, setDraft] = useState("");
  /** The team a buzzer number is being dragged over. */
  const [target, setTarget] = useState<string | null>(null);
  const file = useRef<HTMLInputElement>(null);

  const add = (added: string[]) => added.length > 0 && run(api.addTeams(added));
  const pair = (buzzer: BuzzerId, name: string) => {
    dispatch({ type: "pairTeam", buzzer, name });
    onPick(null);
  };
  const addDraft = () => {
    add(parseTeamNames(draft, false));
    setDraft("");
  };
  const importFile = async (chosen: File) => add(parseTeamNames(decodeText(await chosen.arrayBuffer()), /\.csv$/i.test(chosen.name)));
  const carriesBuzzer = (e: DragEvent) => players !== null && e.dataTransfer.types.includes(BUZZER_TYPE);
  const free = players && players.filter((p) => !p.name).sort((a, b) => a.buzzer - b.buzzer);

  return (
    <aside className="team-panel">
      {free && (
        <>
          <h3>{t("freeBuzzers")}</h3>
          <div className="free-buzzers">
            {free.length > 0 ? (
              free.map((p) => <BuzzerChip key={p.buzzer} buzzer={p.buzzer} free picked={picked === p.buzzer} onPick={onPick} />)
            ) : (
              <span className="muted">{t("noFreeBuzzers")}</span>
            )}
          </div>
        </>
      )}
      <h3>{t("teamNames")}</h3>
      <ul className="team-list">
        {teams.map((team) => (
          <li
            key={team.name}
            className={[target === team.name ? "target" : "", picked !== null ? "pickable" : ""].join(" ")}
            onDragOver={(e) => {
              if (!carriesBuzzer(e)) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = "link";
              setTarget(team.name);
            }}
            onDragLeave={(e) => !e.currentTarget.contains(e.relatedTarget as Node | null) && setTarget(null)}
            onDrop={(e) => {
              e.preventDefault();
              setTarget(null);
              const buzzer = Number(e.dataTransfer.getData(BUZZER_TYPE));
              if (Number.isInteger(buzzer) && buzzer > 0) pair(buzzer, team.name);
            }}
            onClick={() => picked !== null && pair(picked, team.name)}
          >
            <span className="team-name">{team.name}</span>
            {team.buzzer === null ? (
              players && <span className="drop-hint">{t("dropBuzzer")}</span>
            ) : (
              <span className="paired">
                <BuzzerChip buzzer={team.buzzer} free={false} picked={picked === team.buzzer} onPick={onPick} />
                <button
                  className="icon-button"
                  title={t("unpairTeam")}
                  aria-label={t("unpairTeam")}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (team.buzzer !== null) dispatch({ type: "renamePlayer", buzzer: team.buzzer, name: "" });
                  }}
                >
                  <CrossIcon />
                </button>
              </span>
            )}
            <button
              className="icon-button"
              title={t("removeTeam")}
              aria-label={t("removeTeam")}
              onClick={(e) => {
                e.stopPropagation();
                run(api.removeTeam(team.name));
              }}
            >
              <TrashIcon />
            </button>
          </li>
        ))}
      </ul>
      <div className="team-add">
        <input
          value={draft}
          maxLength={MAX_NAME_CHARS}
          placeholder={t("teamNamePlaceholder")}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && addDraft()}
          onPaste={(e) => {
            // Several lines pasted at once are several teams.
            const text = e.clipboardData.getData("text");
            if (!/[\r\n]/.test(text)) return;
            e.preventDefault();
            add(parseTeamNames(text, false));
          }}
        />
        <button onClick={addDraft}>{t("addTeam")}</button>
        <button title={t("importTeamsHint")} onClick={() => file.current?.click()}>
          {t("importTeams")}
        </button>
        <input
          ref={file}
          type="file"
          accept=".txt,.csv,text/plain,text/csv"
          hidden
          onChange={(e) => {
            const chosen = e.target.files?.[0];
            e.target.value = "";
            if (chosen) run(importFile(chosen));
          }}
        />
      </div>
      <p className="muted team-hint">{t(players ? "pairHint" : "teamsBeforeShow")}</p>
    </aside>
  );
}
