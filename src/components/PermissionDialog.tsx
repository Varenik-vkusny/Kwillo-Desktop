import { invoke } from "@tauri-apps/api/core";

interface Props {
  platform: string;
  onDismiss: () => void;
}

export function PermissionDialog({ platform, onDismiss }: Props) {
  async function handleRecord() {
    try {
      await invoke("start_recording");
    } catch (e) {
      console.error("start_recording failed:", e);
    }
  }

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <div style={styles.icon}>🎙</div>
        <div style={styles.title}>Meeting detected</div>
        <div style={styles.platform}>{platform}</div>
        <div style={styles.subtitle}>Record audio for transcription?</div>
        <div style={styles.buttons}>
          <button style={styles.primaryBtn} onClick={handleRecord}>
            Record
          </button>
          <button style={styles.secondaryBtn} onClick={onDismiss}>
            Skip
          </button>
        </div>
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
    margin: 0,
    background: "transparent",
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
  icon: { fontSize: 36, marginBottom: 8 },
  title: {
    color: "#cdd6f4",
    fontSize: 18,
    fontWeight: 600,
    marginBottom: 4,
  },
  platform: {
    color: "#89b4fa",
    fontSize: 14,
    marginBottom: 8,
    fontWeight: 500,
  },
  subtitle: {
    color: "#a6adc8",
    fontSize: 14,
    marginBottom: 24,
  },
  buttons: {
    display: "flex",
    gap: 12,
    justifyContent: "center",
  },
  primaryBtn: {
    background: "#89b4fa",
    color: "#1e1e2e",
    border: "none",
    borderRadius: 8,
    padding: "10px 28px",
    fontSize: 14,
    fontWeight: 600,
    cursor: "pointer",
  },
  secondaryBtn: {
    background: "transparent",
    color: "#a6adc8",
    border: "1px solid rgba(255,255,255,0.15)",
    borderRadius: 8,
    padding: "10px 28px",
    fontSize: 14,
    cursor: "pointer",
  },
};
