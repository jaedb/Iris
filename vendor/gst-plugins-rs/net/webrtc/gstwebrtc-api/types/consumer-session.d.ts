export default ConsumerSession;
declare class ConsumerSession extends WebRTCSession {
    set mungeStereoHack(enable: boolean);
    readonly get streams(): MediaStream[];
    readonly get remoteController(): RemoteController;
    connect(): boolean;
}
import WebRTCSession from "./webrtc-session.js";
import RemoteController from "./remote-controller.js";
