import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

type EventCallback<T> = (payload: T) => void;

export interface TauriEventHandlers {
  onMeetingDetected?: EventCallback<string>;
  onMeetingEnded?: EventCallback<void>;
  onRecordingStarted?: EventCallback<string>;
  onUploadStarted?: EventCallback<void>;
  onUploadSuccess?: EventCallback<void>;
  onUploadFailed?: EventCallback<string>;
}

export function useTauriEvents(handlers: TauriEventHandlers) {
  useEffect(() => {
    const unlisteners: Promise<() => void>[] = [];

    if (handlers.onMeetingDetected) {
      const cb = handlers.onMeetingDetected;
      unlisteners.push(
        listen<string>("meeting-detected", (e) => cb(e.payload))
      );
    }
    if (handlers.onMeetingEnded) {
      const cb = handlers.onMeetingEnded;
      unlisteners.push(listen<void>("meeting-ended", () => cb()));
    }
    if (handlers.onRecordingStarted) {
      const cb = handlers.onRecordingStarted;
      unlisteners.push(
        listen<string>("recording-started", (e) => cb(e.payload))
      );
    }
    if (handlers.onUploadStarted) {
      const cb = handlers.onUploadStarted;
      unlisteners.push(listen<void>("upload-started", () => cb()));
    }
    if (handlers.onUploadSuccess) {
      const cb = handlers.onUploadSuccess;
      unlisteners.push(listen<void>("upload-success", () => cb()));
    }
    if (handlers.onUploadFailed) {
      const cb = handlers.onUploadFailed;
      unlisteners.push(
        listen<string>("upload-failed", (e) => cb(e.payload))
      );
    }

    return () => {
      unlisteners.forEach((p) => p.then((fn) => fn()));
    };
  }, []);
}
