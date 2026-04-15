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

// ─── SVG icons ────────────────────────────────────────────────────────────────

const MicIcon = ({ size = 20 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3Z"/>
    <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
    <line x1="12" x2="12" y1="19" y2="22"/>
  </svg>
);

const SparklesIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>
  </svg>
);

const CheckIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M20 6 9 17l-5-5"/>
  </svg>
);

const AlertIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>
    <path d="M12 9v4"/><path d="M12 17h.01"/>
  </svg>
);

// ─── Component ────────────────────────────────────────────────────────────────

export function RecordingIndicator({ platform, status, errorMessage }: Props) {
  const [elapsed, setElapsed] = useState(0);

  useEffect(() => {
    if (status !== "recording") return;
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, [status]);

  async function handleStop() {
    try {
      await invoke("stop_recording", { uploadUrl: "http://localhost:8080/upload" });
    } catch (e) {
      console.error("stop_recording failed:", e);
    }
  }

  return (
    <div style={S.root}>
      <div className="kwillo-card" style={S.card}>
        {/* Drag region */}
        <div data-tauri-drag-region style={S.dragZone} />

        {status === "recording" && <RecordingView platform={platform} elapsed={elapsed} onStop={handleStop} />}
        {status === "uploading" && <UploadingView />}
        {status === "success"   && <SuccessView />}
        {status === "error"     && <ErrorView message={errorMessage} />}
      </div>
    </div>
  );
}

// ─── State views ──────────────────────────────────────────────────────────────

function RecordingView({ platform, elapsed, onStop }: { platform: string; elapsed: number; onStop: () => void }) {
  return (
    <>
      {/* Pulsing red mic */}
      <div style={S.iconWrap}>
        <div style={{ ...S.ring, borderColor: "rgba(244,63,94,0.3)" }} className="pulse-ring" />
        <div style={{ ...S.iconCircle, background: "rgba(244,63,94,0.08)", border: "1px solid rgba(244,63,94,0.18)" }}>
          <span style={{ color: "#f43f5e" }}><MicIcon size={18} /></span>
        </div>
      </div>

      {/* REC badge + platform */}
      <div style={S.recRow}>
        <span style={S.recDot} />
        <span style={S.recLabel}>REC</span>
        <span style={S.recPlatform}>{platform}</span>
      </div>

      {/* Timer */}
      <div style={S.timer}>{formatDuration(elapsed)}</div>

      {/* Stop button */}
      <button style={S.stopBtn} onClick={onStop}>
        Stop Recording
      </button>
    </>
  );
}

function UploadingView() {
  return (
    <>
      <div style={S.iconWrap}>
        <div style={{ ...S.iconCircle, background: "rgba(0,148,233,0.08)", border: "1px solid rgba(0,148,233,0.18)" }}>
          <span style={{ color: "#0094e9" }}><SparklesIcon /></span>
        </div>
      </div>
      <div style={S.statusTitle}>Uploading...</div>
      <div style={S.statusSub}>Sending audio to Kwillo</div>
      <div style={S.progressTrack}>
        <div style={S.progressFill} className="shimmer-bar" />
      </div>
    </>
  );
}

function SuccessView() {
  return (
    <>
      <div style={S.iconWrap}>
        <div style={{ ...S.iconCircle, background: "rgba(16,185,129,0.08)", border: "1px solid rgba(16,185,129,0.2)" }}>
          <span style={{ color: "#10b981" }}><CheckIcon /></span>
        </div>
      </div>
      <div style={S.statusTitle}>Uploaded</div>
      <div style={{ ...S.statusSub, color: "#10b981" }}>Recording sent successfully</div>
      <div style={S.closingHint}>This window will close shortly.</div>
    </>
  );
}

function ErrorView({ message }: { message?: string }) {
  return (
    <>
      <div style={S.iconWrap}>
        <div style={{ ...S.iconCircle, background: "rgba(245,158,11,0.08)", border: "1px solid rgba(245,158,11,0.2)" }}>
          <span style={{ color: "#f59e0b" }}><AlertIcon /></span>
        </div>
      </div>
      <div style={S.statusTitle}>Upload failed</div>
      <div style={{ ...S.statusSub, color: "#f59e0b" }}>
        {message ?? "An unknown error occurred"}
      </div>
      <div style={S.closingHint}>Check your connection and try again.</div>
    </>
  );
}

// ─── Styles ───────────────────────────────────────────────────────────────────

const S: Record<string, React.CSSProperties> = {
  root: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    height: "100vh",
    padding: "8px",
  },
  card: {
    background: "#0f1117",
    borderRadius: 20,
    border: "1px solid rgba(255,255,255,0.08)",
    boxShadow: "0 20px 60px rgba(0,0,0,0.7), 0 0 0 1px rgba(255,255,255,0.04)",
    padding: "24px 28px 22px",
    width: "100%",
    maxWidth: 360,
    textAlign: "center",
    position: "relative",
    overflow: "hidden",
  },
  dragZone: {
    position: "absolute",
    top: 0, left: 0, right: 0,
    height: 36,
    cursor: "grab",
  },
  iconWrap: {
    position: "relative",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    marginBottom: 12,
  },
  ring: {
    position: "absolute",
    width: 52, height: 52,
    borderRadius: "50%",
    border: "2px solid transparent",
  },
  iconCircle: {
    width: 44, height: 44,
    borderRadius: 14,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },

  // Recording
  recRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: 6,
    marginBottom: 8,
  },
  recDot: {
    width: 6, height: 6,
    borderRadius: "50%",
    background: "#f43f5e",
    display: "inline-block",
    boxShadow: "0 0 6px rgba(244,63,94,0.6)",
  },
  recLabel: {
    fontSize: 10,
    fontWeight: 800,
    color: "#f43f5e",
    letterSpacing: "0.12em",
    textTransform: "uppercase" as const,
  },
  recPlatform: {
    fontSize: 10,
    fontWeight: 600,
    color: "#525a72",
    letterSpacing: "0.05em",
    textTransform: "uppercase" as const,
  },
  timer: {
    fontVariantNumeric: "tabular-nums",
    fontSize: 34,
    fontWeight: 700,
    color: "#f8fafc",
    letterSpacing: "-0.04em",
    marginBottom: 18,
    lineHeight: 1,
  },
  stopBtn: {
    width: "100%",
    height: 38,
    background: "#fff",
    color: "#0f1117",
    border: "none",
    borderRadius: 10,
    fontSize: 12,
    fontWeight: 800,
    letterSpacing: "0.06em",
    textTransform: "uppercase" as const,
    cursor: "pointer",
  },

  // Uploading / Success / Error
  statusTitle: {
    color: "#f8fafc",
    fontSize: 16,
    fontWeight: 700,
    letterSpacing: "-0.03em",
    marginBottom: 4,
  },
  statusSub: {
    color: "#525a72",
    fontSize: 12,
    fontWeight: 500,
    marginBottom: 16,
  },
  progressTrack: {
    height: 3,
    background: "#1a1d27",
    borderRadius: 9999,
    overflow: "hidden",
  },
  progressFill: {
    height: "100%",
    width: "40%",
    background: "linear-gradient(90deg, transparent, #0094e9, transparent)",
    borderRadius: 9999,
  },
  closingHint: {
    color: "#3a4055",
    fontSize: 11,
    marginTop: 12,
  },
};
