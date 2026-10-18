// phygital-key-verify (JavaScript): an independent checker for Phygital Key approvals.
// Works in Node 20+ and modern browsers (WebCrypto). No dependencies.
export { verify, checkKind, MemorySeenStore, keyringFrom, Refused, requestShapeOk, idOk, VERSION, VERSION_BOUND } from "./verify.js";
export { signProof, PROOF_TYP, PROOF_VERSION } from "./bound.js";
export { verifyHistory, verifyHistoryUpdate, fingerprint, keyringAud } from "./history.js";
export { verifyUse, revocationsFrom, MemoryUseStore, standingParams, allows } from "./standing.js";
export { canonical, hashB64, challenge } from "./canonical.js";
export { parse, b64decode, b64encode } from "./strict.js";
