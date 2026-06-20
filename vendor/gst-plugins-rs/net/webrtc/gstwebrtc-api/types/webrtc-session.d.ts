export default WebRTCSession;
declare class WebRTCSession extends EventTarget {
    readonly get peerId(): string;
    readonly get sessionId(): string;
    readonly get state(): SessionState;
    readonly get rtcPeerConnection(): RTCPeerConnection;
    close(): void;
}
import SessionState from "./session-state.js";
