import type {
  CleanupRequest,
  CleanupResponse,
  DictationMode,
  TranscriptionResponse,
} from "@tator/shared-types";

function decodeBase64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const contentType = response.headers.get("content-type") ?? "";
    let message = "";

    if (contentType.includes("application/json")) {
      const body = (await response.json().catch(() => null)) as
        | { detail?: unknown; message?: unknown }
        | null;
      if (typeof body?.detail === "string" && body.detail.length > 0) {
        message = body.detail;
      } else if (typeof body?.message === "string" && body.message.length > 0) {
        message = body.message;
      } else if (body) {
        message = JSON.stringify(body);
      }
    } else {
      message = await response.text();
    }

    throw new Error(message || `Request failed with status ${response.status}`);
  }

  return (await response.json()) as T;
}

export function createApiClient(baseUrl: string) {
  return {
    async transcribe(
      audioBase64: string,
      mode: DictationMode,
      appName?: string | null,
    ): Promise<TranscriptionResponse> {
      const payload = decodeBase64ToBytes(audioBase64);
      const audioBytes = new Uint8Array(payload.byteLength);
      audioBytes.set(payload);
      const formData = new FormData();
      formData.append(
        "file",
        new File([audioBytes], `dictation-${Date.now()}.wav`, {
          type: "audio/wav",
        }),
      );
      formData.append("mode", mode);
      if (appName) {
        formData.append("app_name", appName);
      }

      const response = await fetch(`${baseUrl}/v1/transcribe`, {
        method: "POST",
        body: formData,
      });

      return handleResponse<TranscriptionResponse>(response);
    },

    async cleanup(request: CleanupRequest): Promise<CleanupResponse> {
      const response = await fetch(`${baseUrl}/v1/cleanup`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(request),
      });

      return handleResponse<CleanupResponse>(response);
    },
  };
}
