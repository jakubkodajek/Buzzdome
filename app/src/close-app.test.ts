import { describe, expect, it, vi } from "vitest";
import { answerClose, type CloseDeps } from "./close-app";
import { translator } from "./i18n";
import type { CloseReason, EditorDraft, Quiz } from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn(), message: vi.fn() }));
vi.mock("./api", () => ({ api: {} }));

const t = translator("en");
const draft = { id: "draft-1", quiz: { title: "Pub quiz" } as Quiz } as EditorDraft;
const reason = (show: boolean, unsavedQuiz: boolean): CloseReason => ({ show, unsavedQuiz });

function deps(answers: Partial<CloseDeps> = {}) {
  return {
    loadDraft: vi.fn(async () => draft),
    editorSave: vi.fn(async () => ({ path: "/q/pub.quiz", savesInPlace: true })),
    quitApp: vi.fn(async () => undefined),
    closeDeclined: vi.fn(async () => undefined),
    confirm: vi.fn(async () => true),
    confirmSave: vi.fn(async () => "save" as const),
    failed: vi.fn(async () => undefined),
    ...answers,
  };
}

describe("answerClose", () => {
  it("during a show names the unsaved quiz too, and offers no save", async () => {
    const d = deps();
    await answerClose(t, reason(true, true), d);
    expect(d.confirm).toHaveBeenCalledWith([t("quitShowRuns"), t("quitQuizUnsaved"), t("quitAnyway")].join(" "), t("closeApp"));
    expect(d.loadDraft).not.toHaveBeenCalled();
    expect(d.quitApp).toHaveBeenCalled();
  });

  it("declines when the quizmaster cancels", async () => {
    const d = deps({ confirm: vi.fn(async () => false) });
    await answerClose(t, reason(true, false), d);
    expect(d.closeDeclined).toHaveBeenCalled();
    expect(d.quitApp).not.toHaveBeenCalled();
  });

  it("with an unsaved quiz saves it first, then closes", async () => {
    const d = deps();
    await answerClose(t, reason(false, true), d);
    expect(d.confirmSave).toHaveBeenCalledWith(t("quitSaveQuiz", { title: "Pub quiz" }));
    expect(d.editorSave).toHaveBeenCalledWith("draft-1", draft.quiz, false, expect.objectContaining({ fileName: "Pub quiz" }));
    expect(d.quitApp).toHaveBeenCalled();
  });

  it("closes without saving, or stays when the save is cancelled, fails or the question is", async () => {
    const discard = deps({ confirmSave: vi.fn(async () => "discard" as const) });
    await answerClose(t, reason(false, true), discard);
    expect(discard.editorSave).not.toHaveBeenCalled();
    expect(discard.quitApp).toHaveBeenCalled();

    for (const d of [
      deps({ editorSave: vi.fn(async () => null) }),
      deps({ editorSave: vi.fn(async () => Promise.reject({ code: "noDraft" })) }),
      deps({ confirmSave: vi.fn(async () => "cancel" as const) }),
    ]) {
      await answerClose(t, reason(false, true), d);
      expect(d.quitApp).not.toHaveBeenCalled();
      expect(d.closeDeclined).toHaveBeenCalled();
    }
  });

  it("shows why a save failed", async () => {
    const d = deps({ editorSave: vi.fn(async () => Promise.reject({ code: "noDraft" })) });
    await answerClose(t, reason(false, true), d);
    expect(d.failed).toHaveBeenCalledWith(t("errNoDraft"));
  });
});
