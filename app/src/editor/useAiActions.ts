import { api } from "../api";
import { useApp, useRun } from "../app-context";
import { claudeConnectionText } from "../messages";
import type { Action } from "./actions";

export interface AiActions {
  saveAiSkill: Action;
  connectClaude: Action;
}

/**
 * File › AI assistants (docs/ai-assistants.md): the skill package that chat
 * assistants write quizzes with, and Claude Desktop connected to the app's
 * MCP server. Neither needs an open quiz.
 */
export function useAiActions(): AiActions {
  const { t, notify } = useApp();
  const run = useRun();
  return {
    saveAiSkill: {
      label: t("saveAiSkill"),
      run: () => run(api.aiSaveSkill({ filter: t("skillPackage") }).then((saved) => saved && notify(t("aiSkillSaved")))),
    },
    connectClaude: {
      label: t("connectClaude"),
      run: () => run(api.aiConnectClaude().then((connection) => notify(claudeConnectionText(t, connection)))),
    },
  };
}
