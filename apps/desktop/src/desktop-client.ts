import { invoke } from "@tauri-apps/api/core";

export type OpenAiOAuthMethod = "browser" | "device_code";

export interface OAuthLoginStart {
  session_id: string;
  connection_id: string;
  auth_mode: "chat_gpt_browser_oauth" | "chat_gpt_device_code";
  verification_url: string;
  user_code?: string;
}

export interface ProviderMutation {
  connection: {
    id: string;
    display_name: string;
    auth_mode: string;
  };
  catalog: { models: unknown[] };
}

export function startOpenAiOAuth(method: OpenAiOAuthMethod): Promise<OAuthLoginStart> {
  return invoke("start_openai_oauth", {
    request: {
      connection_id: "openai-subscription",
      display_name: "OpenAI Subscription",
      method
    }
  });
}

export function completeOpenAiOAuth(sessionId: string): Promise<ProviderMutation> {
  return invoke("complete_openai_oauth", { sessionId });
}

export function cancelOpenAiOAuth(sessionId: string): Promise<void> {
  return invoke("cancel_openai_oauth", { sessionId });
}
