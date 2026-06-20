export default defaultConfig;
export type GstWebRTCConfig = {
    meta: object;
    signalingServerUrl: string;
    reconnectionTimeout: number;
    webrtcConfig: object;
};
declare const defaultConfig: GstWebRTCConfig;
