export default ProducerSession;
declare class ProducerSession extends EventTarget {
    readonly get stream(): MediaStream;
    readonly get state(): SessionState;
    start(): boolean;
    close(): void;
}
import SessionState from "./session-state.js";
