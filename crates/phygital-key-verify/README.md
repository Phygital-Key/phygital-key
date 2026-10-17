# phygital-key-verify

Check a [Phygital Key](https://github.com/s17sh70x2jfl031ag/phygital-key-mcp) approval: a person's signed yes, made with a passkey, fingerprint, face or hardware key, for one exact request.

Pure and offline: no network, no clock unless you use `Verifier`, no global state. Fails closed.

```rust
use phygital_key_verify::{Config, Keyring, Level, Verifier};

let verifier = Verifier::new("https://deploy.example.com", keyring, config);
verifier.verify(&approval, &request, Level::High)?; // then act
```

See `examples/basic.rs`, the [spec](https://github.com/s17sh70x2jfl031ag/phygital-key-mcp/blob/main/docs/spec.md) and the shared test vectors. Pre-audit.
