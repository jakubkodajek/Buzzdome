import { describe, expect, it } from "vitest";
import { onlyModel, serverPlace } from "./LocalModelPicker";

const ollama = { name: "Ollama", url: "http://localhost:11434/v1", models: ["a"] };

describe("the model picker", () => {
  it("names a server by its host and port", () => {
    expect(serverPlace("http://localhost:11434/v1")).toBe("localhost:11434");
    expect(serverPlace("https://box.local:8080/v1/")).toBe("box.local:8080");
  });

  it("uses the only model found while none is chosen, and never replaces a choice", () => {
    expect(onlyModel([ollama], null)).toEqual({ url: ollama.url, model: "a" });
    expect(onlyModel([ollama, { name: null, url: "http://localhost:8080/v1", models: [] }], null)).toEqual({ url: ollama.url, model: "a" });
    expect(onlyModel([{ ...ollama, models: ["a", "b"] }], null)).toBeNull();
    expect(onlyModel([ollama, { name: "LM Studio", url: "http://localhost:1234/v1", models: ["c"] }], null)).toBeNull();
    expect(onlyModel([], null)).toBeNull();
    expect(onlyModel([ollama], { url: "http://localhost:1234/v1", model: "c" })).toBeNull();
  });
});
