// File › AI assistants (docs/ai-assistants.md): the folder AI apps work in,
// the apps connected to the app's MCP server, and the quizmaster's own
// instructions for every quiz. None of it needs an open quiz.

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { useApp, useRun } from "../app-context";
import type { StringKey } from "../i18n";
import { aiConnectionText, aiRestartText, settingsLostNotice } from "../messages";
import { NoticeBox } from "../notice";
import type { AiPage as AiPageInfo, AssistantInfo, ConnectionState } from "../types";

const STATE_KEYS: Record<ConnectionState, StringKey> = {
  connected: "aiConnectedState",
  notConnected: "aiNotConnectedState",
  outdated: "aiOutdatedState",
};

export function AiPage() {
  const { t, lang, notify, warn } = useApp();
  const run = useRun();
  const [page, setPage] = useState<AiPageInfo | null>(null);
  /** The instructions as the quizmaster is changing them; null while they are as saved. */
  const [draft, setDraft] = useState<string | null>(null);
  /** The settings for another AI app, once the quizmaster asked for them. */
  const [config, setConfig] = useState<string | null>(null);
  const load = useCallback(() => api.aiPage(lang).then(setPage), [lang]);
  useEffect(() => run(load()), [run, load]);
  if (!page) return <h1>{t("backstageAi")}</h1>;

  const saved = page.instructions ?? page.defaultInstructions;
  const instructions = draft ?? saved;
  const chooseFolder = () => run(api.aiChooseFolder().then((folder) => (folder === null ? undefined : load())));
  const connect = (info: AssistantInfo) =>
    run(api.aiConnect(info.assistant, lang).then((connected) => load().then(() => notify(aiConnectionText(t, connected, page.platform)))));
  const copyConfig = () =>
    run(
      api.aiServerConfig(lang).then(async (text) => {
        setConfig(text);
        const copied = await navigator.clipboard.writeText(text).then(
          () => true,
          () => false,
        );
        if (copied) notify(t("aiConfigCopied"));
        else warn(t("aiConfigNotCopied"));
      }),
    );
  /** Keeps `text` as the quizmaster's instructions; empty gives the default ones back. */
  const saveInstructions = (text: string) =>
    run(
      api
        .aiSetInstructions(text, lang)
        .then(load)
        .then(() => {
          setDraft(null);
          notify(t("aiInstructionsSaved"));
        }),
    );

  return (
    <>
      <h1>{t("backstageAi")}</h1>
      <p className="backstage-hint">{t("aiAssistantsHint")}</p>
      {page.settingsError && (
        <NoticeBox notice={settingsLostNotice(t, t("aiSettingsLost"), page.settingsError)} />
      )}

      <h2>{t("aiFolderTitle")}</h2>
      <div className="backstage-task">
        <button type="button" className="browse" onClick={chooseFolder}>
          {t("aiChooseFolder")}
        </button>
        <div>
          <p className="ai-folder">{page.folder ?? t("aiNoFolder")}</p>
          <p>{t("aiFolderHint")}</p>
        </div>
      </div>

      <h2>{t("aiAppsTitle")}</h2>
      <p className="backstage-hint">{t("aiAppsHint")}</p>
      <div className="ai-apps">
        {page.assistants.map((info) => {
          const installed = info.configFiles.length > 0;
          return (
            <div key={info.assistant} className="ai-app">
              <strong>{info.name}</strong>
              <span className={info.state === "connected" ? "chip" : info.state === "outdated" ? "chip warn" : "chip off"}>
                {installed ? t(STATE_KEYS[info.state]) : t("aiNotInstalled")}
              </span>
              <button type="button" className="browse" data-assistant={info.assistant} disabled={!installed} onClick={() => connect(info)}>
                {t(info.state === "notConnected" ? "aiConnect" : "aiConnectAgain")}
              </button>
              {installed && (
                <p className="ai-app-hint">
                  {t("aiSettingsFile", { path: info.configFiles.join(", ") })} {aiRestartText(t, info.assistant, page.platform)}
                </p>
              )}
            </div>
          );
        })}
      </div>
      <h3>{t("aiOtherApp")}</h3>
      <div className="backstage-task">
        <button type="button" className="browse" onClick={copyConfig}>
          {t("aiCopyConfig")}
        </button>
        <p>{t("aiOtherAppHint")}</p>
      </div>
      {config !== null && <pre className="ai-config">{config}</pre>}

      <h2>{t("aiInstructionsTitle")}</h2>
      <p className="backstage-hint">{t("aiInstructionsHint")}</p>
      <textarea
        className="ai-instructions"
        aria-label={t("aiInstructionsTitle")}
        value={instructions}
        maxLength={page.maxInstructionsChars}
        rows={14}
        onChange={(e) => setDraft(e.target.value)}
      />
      <div className="ai-instructions-buttons">
        <button type="button" className="browse" disabled={instructions === saved} onClick={() => saveInstructions(instructions)}>
          {t("aiSaveInstructions")}
        </button>
        <button type="button" className="browse" disabled={page.instructions === null && instructions === saved} onClick={() => saveInstructions("")}>
          {t("aiDefaultInstructions")}
        </button>
      </div>
    </>
  );
}
