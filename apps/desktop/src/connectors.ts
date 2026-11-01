import type { CloudProviderV1 } from "./contracts";

export const providerNames: Record<CloudProviderV1, string> = {
  chatgpt: "ChatGPT subscription",
  openai: "OpenAI",
  gemini: "Gemini",
};
