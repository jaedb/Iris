export default RemoteController;
declare class RemoteController extends EventTarget {
    readonly get rtcDataChannel(): RTCDataChannel;
    readonly get consumerSession(): ConsumerSession;
    readonly get videoElement(): HTMLVideoElement;
    attachVideoElement(element: HTMLVideoElement | null): void;
    sendControlRequest(request: object | string): number;
    close(): void;
}
import type ConsumerSession from "./consumer-session.js";
