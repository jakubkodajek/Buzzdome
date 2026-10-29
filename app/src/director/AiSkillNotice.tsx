import { useEffect, useState } from "react";
import { api } from "../api";
import { useApp } from "../app-context";
import { useSaveAiSkill } from "../editor/useAiActions";

/**
 * Says, while no show runs, that the AI skill the quizmaster saved before
 * differs from this version's, as after an update that changed it: the copy
 * uploaded to an assistant stays as it was (docs/ai-assistants.md). Saving
 * it again ends the notice.
 */
export function AiSkillNotice({ showRunning }: { showRunning: boolean }) {
  const { t } = useApp();
  const saveSkill = useSaveAiSkill();
  const [outdated, setOutdated] = useState(false);
  useEffect(() => {
    let alive = true;
    api
      .aiSkillOutdated()
      .then((found) => {
        if (alive) setOutdated(found);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);
  if (!outdated || showRunning) return null;
  const save = () => void saveSkill().then((saved) => saved && setOutdated(false));
  return (
    <div className="update-banner" role="status">
      <span>{t("aiSkillOutdated")}</span>
      <button onClick={save}>{t("saveAiSkill")}</button>
    </div>
  );
}
