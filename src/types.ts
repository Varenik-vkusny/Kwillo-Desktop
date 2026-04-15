export type AppView = "hidden" | "permission" | "recording" | "uploading";

export interface MeetingDetectedPayload {
  platform: string;
}

export interface RecordingStartedPayload {
  platform: string;
}
