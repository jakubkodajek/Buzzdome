// The model on this computer that Studio's AI assistant asks: the app looks
// for the model servers running here (`ai_find_local_models`) and lists
// their models, so the quizmaster only picks one. Shown in File › AI
// assistants and in the AI panel until a model is chosen.

import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { CheckIcon } from "../icons";
import type { FoundServer, LocalModel } from "../types";

interface LocalModelPickerProps {
  /** The model chosen now; none until the quizmaster chose one. */
  chosen: LocalModel | null;
  onChosen: (chosen: LocalModel) => void;
  /** Uses the only model found while none is chosen, where the quizmaster
   * is about to ask (the AI panel); elsewhere a model is only listed. */
  autoUse?: boolean;
}

/** Where a server is, as people know it: its host and port. */
export function serverPlace(url: string): string {
  return url.replace(/^https?:\/\//i, "").replace(/\/v1\/?$/i, "");
}

/** The model to use without asking: the only one found, while none is chosen. */
export function onlyModel(found: readonly FoundServer[], chosen: LocalModel | null): LocalModel | null {
  if (chosen) return null;
  const all = found.flatMap((server) => server.models.map((model) => ({ url: server.url, model })));
  return all.length === 1 ? all[0] : null;
}

export function LocalModelPicker({ chosen, onChosen, autoUse = false }: LocalModelPickerProps) {
  const { t, notify } = useApp();
  const run = useRun();
  const [found, setFound] = useState<FoundServer[] | null>(null);
  // Only the latest search fills the list.
  const asked = useRef(0);
  // The choice as it is when a search answers, not as it was when it began.
  const chosenNow = useRef(chosen);
  useEffect(() => {
    chosenNow.current = chosen;
  }, [chosen]);
  const use = useCallback(
    (url: string, model: string) =>
      api.aiSetLocalModel(url, model).then((saved) => {
        onChosen(saved);
        notify(t("aiLocalSaved", { model: saved.model }));
      }),
    [onChosen, notify, t],
  );
  const search = useCallback(() => {
    const request = ++asked.current;
    setFound(null);
    return api.aiFindLocalModels().then(
      (servers) => {
        if (request !== asked.current) return;
        setFound(servers);
        const only = autoUse ? onlyModel(servers, chosenNow.current) : null;
        if (only) return use(only.url, only.model);
      },
      (error: unknown) => {
        // A failed search lists nothing, so it can be tried again.
        if (request === asked.current) setFound([]);
        throw error;
      },
    );
  }, [autoUse, use]);
  // Looked for once as the picker opens, and again on Search again: not each time a model is chosen.
  const first = useRef(() => run(search()));
  useEffect(() => first.current(), []);

  const servers = found ?? [];
  const withModels = servers.filter((server) => server.models.length > 0);
  const empty = servers.filter((server) => server.models.length === 0);
  return (
    <div className="ai-servers" aria-busy={found === null}>
      {found === null && <p className="ai-servers-note">{t("aiLocalSearching")}</p>}
      {withModels.map((server) => (
        <section key={server.url} className="ai-server" aria-label={server.name ?? serverPlace(server.url)}>
          <h3>
            {server.name ?? t("aiLocalServer")} <span>{serverPlace(server.url)}</span>
          </h3>
          <ul>
            {server.models.map((model) => {
              const inUse = chosen?.url === server.url && chosen.model === model;
              return (
                <li key={model} className={inUse ? "in-use" : undefined}>
                  <span className="ai-server-model">{model}</span>
                  {inUse ? (
                    <span className="ai-server-in-use">
                      <CheckIcon /> {t("aiLocalInUseShort")}
                    </span>
                  ) : (
                    <button type="button" className="browse" onClick={() => run(use(server.url, model))}>
                      {t("aiLocalUse")}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      {empty.map((server) => (
        <p key={server.url} className="ai-servers-note">
          {t("aiLocalServerEmpty", { server: server.name ?? serverPlace(server.url) })}
        </p>
      ))}
      {found !== null && servers.length === 0 && <p className="ai-servers-note">{t("aiLocalNoneFound")}</p>}
      <button type="button" className="browse" disabled={found === null} onClick={() => run(search())}>
        {t("aiLocalSearchAgain")}
      </button>
    </div>
  );
}
