import { invoke } from "@tauri-apps/api/core";

interface Props {
  platform: string;
  onDismiss: () => void;
}

// Inline SVG icons — no external dependency needed
const MicIcon = () => (
  <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3Z"/>
    <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
    <line x1="12" x2="12" y1="19" y2="22"/>
  </svg>
);

export function PermissionDialog({ platform, onDismiss }: Props) {
  async function handleRecord() {
    try {
      await invoke("start_recording");
    } catch (e) {
      console.error("start_recording failed:", e);
    }
  }

  return (
    <div style={S.root}>
      <div className="kwillo-card" style={S.card}>
        {/* Drag region — top strip so user can move the popup */}
        <div data-tauri-drag-region style={S.dragZone} />

        {/* Mic icon */}
        <div style={S.iconWrap}>
          <div style={S.iconRing} className="pulse-ring" />
          <div style={S.iconCircle}>
            <span style={{ color: "#0094e9" }}><MicIcon /></span>
          </div>
        </div>

        {/* Texts */}
        <div style={S.title}>Meeting detected</div>
        <div style={S.platformBadge}>{platform}</div>
        <div style={S.subtitle}>Record audio for this call?</div>

        {/* Buttons */}
        <div style={S.btnRow}>
          <button style={S.primaryBtn} onClick={handleRecord}>
            Record
          </button>
          <button style={S.ghostBtn} onClick={onDismiss}>
            Skip
          </button>
        </div>
      </div>
    </div>
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
    padding: "28px 28px 24px",
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
    marginBottom: 16,
  },
  iconRing: {
    position: "absolute",
    width: 52,
    height: 52,
    borderRadius: "50%",
    border: "2px solid rgba(0,148,233,0.3)",
  },
  iconCircle: {
    width: 44,
    height: 44,
    borderRadius: 14,
    background: "rgba(0,148,233,0.08)",
    border: "1px solid rgba(0,148,233,0.18)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  title: {
    color: "#f8fafc",
    fontSize: 16,
    fontWeight: 700,
    letterSpacing: "-0.03em",
    marginBottom: 8,
  },
  platformBadge: {
    display: "inline-block",
    background: "rgba(0,148,233,0.1)",
    border: "1px solid rgba(0,148,233,0.2)",
    color: "#0094e9",
    borderRadius: 6,
    padding: "2px 10px",
    fontSize: 11,
    fontWeight: 700,
    letterSpacing: "0.05em",
    textTransform: "uppercase",
    marginBottom: 14,
  },
  subtitle: {
    color: "#525a72",
    fontSize: 13,
    fontWeight: 500,
    marginBottom: 20,
  },
  btnRow: {
    display: "flex",
    gap: 8,
  },
  primaryBtn: {
    flex: 1,
    height: 38,
    background: "#0094e9",
    color: "#fff",
    border: "none",
    borderRadius: 10,
    fontSize: 13,
    fontWeight: 700,
    cursor: "pointer",
    letterSpacing: "-0.01em",
    transition: "background 0.15s, transform 0.1s",
  },
  ghostBtn: {
    flex: 1,
    height: 38,
    background: "transparent",
    color: "#525a72",
    border: "1px solid rgba(255,255,255,0.08)",
    borderRadius: 10,
    fontSize: 13,
    fontWeight: 600,
    cursor: "pointer",
    transition: "border-color 0.15s, color 0.15s",
  },
};
