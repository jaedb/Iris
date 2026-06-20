export default SessionState;
type SessionState = number;
declare namespace SessionState {
    let idle: number;
    let connecting: number;
    let streaming: number;
    let closed: number;
}
