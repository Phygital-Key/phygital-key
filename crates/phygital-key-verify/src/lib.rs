//! # phygital-key-verify
//!
//! Check a Phygital Key approval: a person's signed yes, made with a passkey,
//! fingerprint, face or hardware key, for one exact request.
//!
//! The check is pure and offline. It fails closed: anything missing, expired,
//! reused, changed or unclear is a no.
//!
//! See the spec in `docs/spec.md` of the repository.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod b64;
pub mod canonical;
mod model;
pub mod strict;
mod verify;
pub mod webauthn;

pub use model::{
    ApprovalObject, Assertion, Claims, Config, DEFAULT_LIFETIME_SECS, Expected, KeyCard, Keyring, Level,
    MAX_APPROVAL_BYTES, MAX_CLOCK_SKEW_SECS, MAX_LIFETIME_SECS, MAX_REQUEST_BYTES, VERSION, id_ok,
    request_shape_ok,
};
pub use verify::{ErrorCode, MemorySeenStore, SeenStore, Verified, Verifier, check, verify};
