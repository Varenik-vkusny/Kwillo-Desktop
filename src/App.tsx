import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTauriEvents } from "./hooks/useTauriEvents";
import { PermissionDialog } from "./components/PermissionDialog";
import { RecordingIndicator } from "./components/RecordingIndicator";

type AppView = "hidden" | "permission" | "recording" | "uploading";

export default function App() {
  const [view, setView] = useState<AppView>("hidden");
  const [platform, setPlatform] = useState("");
  const [uploadStatus, setUploadStatus] = useState<
    "recording" | "uploading" | "success" | "error"
  >("recording");
  const [errorMessage, setErrorMessage] = useState<string | undefined>();

  useTauriEvents({
    onMeetingDetected: (p) => {
      setPlatform(p);
      setView("permission");
    },
    onMeetingEnded: () => {
      setUploadStatus("uploading");
    },
    onRecordingStarted: (p) => {
      setPlatform(p);
      setUploadStatus("recording");
      setView("recording");
    },
    onUploadStarted: () => {
      setUploadStatus("uploading");
    },
    onUploadSuccess: () => {
      setUploadStatus("success");
      setTimeout(async () => {
        setView("hidden");
        const win = await getCurrentWindow();
        await win.hide();
      }, 2000);
    },
    onUploadFailed: (msg) => {
      setUploadStatus("error");
      setErrorMessage(msg);
    },
    // Backend auto-dismissed the dialog (meeting ended while prompt was open,
    // or the 60-second timeout expired). Reset the UI to its idle state.
    onPermissionDismissed: () => {
      setView("hidden");
    },
  });

  if (view === "hidden") return null;

  if (view === "permission") {
    return (
      <PermissionDialog
        platform={platform}
        onDismiss={async () => {
          try { await invoke("dismiss_recording"); } catch {}
          setView("hidden");
          const win = await getCurrentWindow();
          await win.hide();
        }}
      />
    );
  }

  return (
    <RecordingIndicator
      platform={platform}
      status={uploadStatus}
      errorMessage={errorMessage}
    />
  );
}
