// File › AI assistants (docs/ai-assistants.md): the folder AI apps work in,
// the apps connected to the app's MCP server, and the quizmaster's own
// instructions for every quiz. None of it needs an open quiz.

import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { confirmAction } from "../confirm";
import { useApp, useRun } from "../app-context";
import type { StringKey } from "../i18n";
import { aiConnectionText, aiRestartText, errorNotice, settingsLostNotice } from "../messages";
import { NoticeBox } from "../notice";
import { readPreference, writePreference } from "../preferences";
import { createStore, useStore } from "../store";
import type { AiPage as AiPageInfo, AssistantInfo, ConnectionState } from "../types";

const STATE_KEYS: Record<ConnectionState, StringKey> = {
  connected: "aiConnectedState",
  notConnected: "aiNotConnectedState",
  outdated: "aiOutdatedState",
};

/** Where the instructions being typed are kept on this computer, so closing the app does not lose them. */
const DRAFT_KEY = "quiz-show.ai-instructions-draft";

/**
 * The instructions as the quizmaster is changing them, null while they are
 * as saved: kept while another page is shown, and over a restart.
 */
const instructionsDraft = createStore<string | null>(storedDraft());
instructionsDraft.subscribe(() => writePreference(DRAFT_KEY, JSON.stringify(instructionsDraft.get())));

function storedDraft(): string | null {
  try {
    const stored: unknown = JSON.parse(readPreference(DRAFT_KEY) ?? "null");
    return typeof stored === "string" ? stored : null;
  } catch {
    return null;
  }
}

export function AiPage() {
  const { t, lang, notify, fail } = useApp();
  const run = useRun();
  const [page, setPage] = useState<AiPageInfo | null>(null);
  /** Why the page could not be read, until it is tried again. */
  const [failed, setFailed] = useState<{ error: unknown } | null>(null);
  const typed = useStore(instructionsDraft);
  /** The settings for another AI app, once the quizmaster asked for them. */
  const [config, setConfig] = useState<string | null>(null);
  const load = useCallback(() => api.aiPage(lang).then(setPage), [lang]);
  // Said on the page, which shows nothing else until it reads.
  const show = useCallback(() => load().catch((error: unknown) => setFailed({ error })), [load]);
  useEffect(() => void show(), [show]);
  const saved = page && (page.instructions ?? page.defaultInstructions);
  // A draft kept from before that matches the saved instructions (saved since, say) is nothing unsaved.
  const draft = typed === saved ? null : typed;
  if (!page || saved === null) {
    return (
      <>
        <h1>{t("backstageAi")}</h1>
        {failed ? (
          <div className="backstage-task">
            <NoticeBox notice={errorNotice(t, failed.error)} />
            <button
              type="button"
              className="browse"
              onClick={() => {
                setFailed(null);
                void show();
              }}
            >
              {t("tryAgain")}
            </button>
          </div>
        ) : (
          <p className="backstage-hint">{t("loading")}</p>
        )}
      </>
    );
  }

  const instructions = draft ?? saved;
  const setDraft = (text: string) => instructionsDraft.set(text === saved ? null : text);
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
        else fail(t("aiConfigNotCopied"));
      }),
    );
  /** Keeps `text` as the quizmaster's instructions; empty gives the default ones back, which `done` says. */
  const saveInstructions = (text: string, done: StringKey = "aiInstructionsSaved") =>
    run(
      api
        .aiSetInstructions(text, lang)
        .then(load)
        .then(() => {
          instructionsDraft.set(null);
          notify(t(done));
        }),
    );
  // Offered only while there are own instructions, saved or being typed, which it deletes for good: asked first.
  const restoreDefault = () =>
    void confirmAction(t, t("aiDefaultInstructionsConfirm"), t("aiDefaultInstructions")).then((go) => go && saveInstructions("", "aiDefaultInstructionsDone"));

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

      <h2>
        {t("aiInstructionsTitle")} {draft !== null && <span className="chip warn">{t("unsaved")}</span>}
      </h2>
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
        <button type="button" className="browse" disabled={page.instructions === null && instructions === saved} onClick={restoreDefault}>
          {t("aiDefaultInstructions")}
        </button>
      </div>
    </>
  );
}
