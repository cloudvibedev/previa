import { create } from "zustand";

export type OpenAIModel = "gpt-5.4" | "gpt-5.2" | "gpt-5-mini" | "gpt-5-nano";

export const OPENAI_MODELS: { value: OpenAIModel; label: string; description: string }[] = [
  { value: "gpt-5.4", label: "GPT-5.4", description: "Último modelo, velocidade rápida e raciocínio alto" },
  { value: "gpt-5.2", label: "GPT-5.2", description: "Mais poderoso, melhor raciocínio" },
  { value: "gpt-5-mini", label: "GPT-5 Mini", description: "Equilíbrio entre custo e qualidade" },
  { value: "gpt-5-nano", label: "GPT-5 Nano", description: "Mais rápido e econômico" },
];

interface OpenAIKeyState {
  apiKey: string | null;
  model: OpenAIModel;
  setApiKey: (key: string | null) => void;
  setModel: (model: OpenAIModel) => void;
}

export const useOpenAIKeyStore = create<OpenAIKeyState>((set) => ({
  apiKey: localStorage.getItem("openai-api-key"),
  model: (localStorage.getItem("openai-model") as OpenAIModel) || "gpt-5.4",
  setApiKey: (key) => {
    if (key) {
      localStorage.setItem("openai-api-key", key);
    } else {
      localStorage.removeItem("openai-api-key");
    }
    set({ apiKey: key });
  },
  setModel: (model) => {
    localStorage.setItem("openai-model", model);
    set({ model });
  },
}));
