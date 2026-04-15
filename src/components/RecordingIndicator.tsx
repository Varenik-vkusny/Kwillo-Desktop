import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  platform: string;
  status: "recording" | "uploading" | "success" | "error";
  errorMessage?: string;
}

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return [h, m, s].map((v) => String(v).padStart(2, "0")).join(":");
}

export function RecordingIndicator({ platform, status, errorMessage }: Props) {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (status !== "recording") return;
    const interval = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(interval);
  }, [status]);

  async function handleStop() {
    try {
      await invoke("stop_recording", {
        uploadUrl: "http://localhost:8080/upload",
      });
    } catch (e) {
      console.error("stop_recording failed:", e);
    }
  }

  const statusLabel =
    status === "recording"
      ? `● Recording  ${formatDuration(elapsed)}`
      : status === "uploading"
      ? "⏫ Uploading..."
      : status === "success"
      ? "✓ Sent"
      : `✗ Error: ${errorMessage ?? "unknown"}`;

  const statusColor =
    status === "recording"
      ? "#f38ba8"
      : status === "uploading"
      ? "#fab387"
      : status === "success"
      ? "#a6e3a1"
      : "#f38ba8";

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <div style={{ ...styles.statusLabel, color: statusColor }}>
          {statusLabel}
        </div>
        <div style={styles.platform}>{platform}</div>
        {status === "recording" && (
          <button style={styles.stopBtn} onClick={handleStop}>
            Stop Recording
          </button>
        )}
        {(status === "success" || status === "error") && (
          <div style={styles.doneHint}>This window will close shortly.</div>
        )}
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    height: "100vh",
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  },
  card: {
    background: "#1e1e2e",
    borderRadius: 16,
    padding: "28px 32px",
    width: 360,
    textAlign: "center",
    boxShadow: "0 8px 32px rgba(0,0,0,0.4)",
    border: "1px solid rgba(255,255,255,0.08)",
  },
  statusLabel: {
    fontSize: 18,
    fontWeight: 600,
    marginBottom: 8,
  },
  platform: {
    color: "#89b4fa",
    fontSize: 14,
    marginBottom: 20,
  },
  stopBtn: {
    background: "#f38ba8",
    color: "#1e1e2e",
    border: "none",
    borderRadius: 8,
    padding: "10px 28px",
    fontSize: 14,
    fontWeight: 600,
    cursor: "pointer",
  },
  doneHint: {
    color: "#585b70",
    fontSize: 12,
    marginTop: 12,
  },
};
