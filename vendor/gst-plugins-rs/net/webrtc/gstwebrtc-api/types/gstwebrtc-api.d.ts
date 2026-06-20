export default GstWebRTCAPI;
declare class GstWebRTCAPI {
    constructor(userConfig?: import("./config.js").GstWebRTCConfig);
    registerConnectionListener(listener: ConnectionListener): boolean;
    unregisterConnectionListener(listener: ConnectionListener): boolean;
    unregisterAllConnectionListeners(): void;
    createProducerSession(stream: MediaStream): ProducerSession;
    createProducerSessionForConsumer(stream: MediaStream, consumerId: string): ProducerSession;
    getAvailableProducers(): Peer[];
    getAvailableConsumers(): Peer[];
    registerPeerListener(listener: PeerListener): boolean;
    unregisterPeerListener(listener: PeerListener): boolean;
    unregisterAllPeerListeners(): void;
    createConsumerSession(producerId: string): ConsumerSession;
    createConsumerSessionWithOfferOptions(producerId: string, offerOptions: RTCOfferOptions): ConsumerSession;
}
declare namespace GstWebRTCAPI {
    export { SessionState };
}
import type ProducerSession from "./producer-session.js";
import type ConsumerSession from "./consumer-session.js";
import SessionState from "./session-state.js";

/* Added manually */
export interface ConnectionListener {
    connected(clientId: string): void;
    disconnected(): void;
}
export interface PeerListener {
    producerAdded?(producer: Peer): void;
    producerRemoved?(producer: Peer): void;
    consumerAdded?(consumer: Peer): void;
    consumerRemoved?(consumer: Peer): void;
}
export type Peer = {
    readonly id: string;
    readonly meta: Record<string, unknown>;
};
