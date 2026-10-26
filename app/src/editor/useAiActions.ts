import { api } from "../api";
import { useApp, useReport, useRun } from "../app-context";
import { claudeConnectionText } from "../messages";
import type { Action } from "./actions";

/**
 * Saves the skill package where the quizmaster picks and says so; resolves
 * whether it was saved (false when they cancel or it failed, which is reported).
 */
export function useSaveAiSkill(): () => Promise<boolean> {
  const { t, notify } = useApp();
  const report = useReport();
  return () =>
    api
      .aiSaveSkill({ filter: t("skillPackage") })
      .then((saved) => {
        if (saved) notify(t("aiSkillSaved"));
        return saved;
      })
      .catch((error: unknown) => {
        report(error);
        return false;
      });
}

export interface AiActions {
  saveAiSkill: Action;
  connectClaude: Action;
  connectClaudeFolder: Action;
}

/**
 * File › AI assistants (docs/ai-assistants.md): the skill package that chat
 * assistants write quizzes with, and Claude Desktop connected to the app's
 * MCP server, which works only in the folder chosen here. Neither needs an
 * open quiz.
 */
export function useAiActions(): AiActions {
  const { t, notify } = useApp();
  const run = useRun();
  const saveSkill = useSaveAiSkill();
  /** Connects Claude Desktop with the documents' Buzzdome folder, or with `choose` one the quizmaster picks. */
  const connectClaude = (label: string, choose: boolean): Action => ({
    label,
    run: () => run(api.aiConnectClaude(choose).then((connected) => connected && notify(claudeConnectionText(t, connected)))),
  });
  return {
    saveAiSkill: {
      label: t("saveAiSkill"),
      run: () => void saveSkill(),
    },
    connectClaude: connectClaude(t("connectClaude"), false),
    connectClaudeFolder: connectClaude(t("connectClaudeFolder"), true),
  };
}
