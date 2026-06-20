export enum MitigationMode {
  None = 0,
  Downscaled = 1,
  Downsampled = 2,
}

export interface ConsumerType {
  id: string,
  video_codec: string | undefined,
  mitigation_mode: MitigationMode,
  stats: Map<string, number>,
}

export enum WebSocketStatus {
  Connecting = 0,
  Connected = 1,
  Error = 2,
}
