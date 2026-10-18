//! What a product built on the gate tells the gate about itself.
//!
//! The gate (approval page, keys, standing approvals, receipts, audit log, secure
//! install, signed updates and the shared commands) is the same in every Phygital
//! Key product. A product adds one thing: the agent protocol it speaks (MCP, A2A),
//! as an [`Adapter`], plus its own name, port and release channel ([`Product`]).

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;

use crate::state::{Settings, Shared};

/// A product's identity. Everything here is fixed at build time.
#[derive(Debug)]
pub struct Product {
    /// Display name, e.g. `Phygital Key MCP`.
    pub name: &'static str,
    /// Program name, e.g. `phygital-key-mcp`. Also the release file prefix, the
    /// data folder (`~/.<bin>`) and the environment prefix (`PHYGITAL_KEY_MCP_`).
    pub bin: &'static str,
    /// Short id for the secure install's user, folders and service labels, e.g. `mcp`.
    /// Lowercase letters and digits only.
    pub id: &'static str,
    /// Program version (the product crate's `CARGO_PKG_VERSION`).
    pub version: &'static str,
    /// GitHub repository with the product's signed releases, `owner/name`.
    pub repo: &'static str,
    /// Default port of the approval page.
    pub port: u16,
    /// One line for `--help`.
    pub about: &'static str,
    /// The product's pinned release keys: P-256 `PUBLIC KEY` PEM blocks, and
    /// `ML-DSA-65 PUBLIC KEY` blocks once a post-quantum key is made.
    pub release_keys: &'static str,
}

impl Product {
    /// True when every field is safe to write into the installers and units
    /// (letters, digits and a few separators; no quotes, `$`, newlines).
    pub fn is_valid(&self) -> bool {
        let only = |s: &str, ok: fn(char) -> bool| !s.is_empty() && s.chars().all(ok);
        only(self.name, |c| c.is_ascii_alphanumeric() || c == ' ')
            && only(self.bin, |c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
            })
            && only(self.id, |c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && self.repo.split_once('/').is_some_and(|(o, n)| {
                only(o, |c| c.is_ascii_alphanumeric() || c == '-')
                    && only(n, |c| {
                        c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
                    })
            })
            && crate::update::Version::parse(self.version).is_some()
    }

    /// The environment variable for one setting, e.g. `PHYGITAL_KEY_MCP_PORT`.
    pub fn env(&self, what: &str) -> String {
        format!("{}_{what}", self.bin.to_ascii_uppercase().replace('-', "_"))
    }
}

/// A future the gate can run to completion (an adapter's stdio mode).
pub type Task = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;

/// The agent protocol a product speaks.
pub trait Adapter: Send + Sync + 'static {
    /// URL path the protocol is served on, e.g. `/mcp`. Every request under it
    /// needs the access token and is refused when it carries a browser `Origin`.
    fn path(&self) -> &'static str;
    /// The protocol's routes, all under [`Adapter::path`] (the adapter mounts them).
    fn router(&self, gate: Shared) -> axum::Router;
    /// Routes anyone may read without the token, for example an A2A agent card.
    /// GET only; they never change state.
    fn public_router(&self, _gate: Shared) -> Option<axum::Router> {
        None
    }
    /// How to connect an AI app or agent (shown by `serve`, `demo` and `token`).
    fn connect_hint(&self, settings: &Settings) -> String;
    /// What to try first in the demo.
    fn demo_hint(&self) -> &'static str;
    /// True when the protocol has a stdio mode ([`Adapter::stdio`]).
    fn has_stdio(&self) -> bool {
        false
    }
    /// Run the gate for one client over stdio, if the protocol has a stdio mode.
    fn stdio(&self, _gate: Shared) -> Option<Task> {
        None
    }
}

/// Used when no product has been set (tests and tools inside this crate).
pub const UNSET: Product = Product {
    name: "Phygital Key",
    bin: "phygital-key",
    id: "dev",
    version: env!("CARGO_PKG_VERSION"),
    repo: "Phygital-Key/phygital-key",
    port: 7468,
    about: "A person approves exact AI actions with a passkey or hardware key.",
    release_keys: "",
};

static PRODUCT: OnceLock<&'static Product> = OnceLock::new();

/// The running product (set once by [`crate::run`]).
pub fn product() -> &'static Product {
    PRODUCT.get().copied().unwrap_or(&UNSET)
}

pub(crate) fn set(p: &'static Product) {
    let _ = PRODUCT.set(p);
}

const INSTALL_SERVICE: &str = include_str!("../assets/install-service.sh");
const INSTALL_USER: &str = include_str!("../assets/install.sh");
const INSTALL_TEST: &str = include_str!("../assets/secure-install-test.sh");
const RELEASE_KEY: &str = include_str!("../assets/release-key.sh");
const MANIFESTS: &str = include_str!("../assets/package-manifests.sh");

/// Replace the text between `# BEGIN <name>` and `# END <name>` (the marker lines
/// stay). Returns the template unchanged when the block is missing.
fn fill(template: &str, name: &str, body: &str) -> String {
    let (begin, end) = (format!("# BEGIN {name}\n"), format!("# END {name}"));
    match (template.find(&begin), template.find(&end)) {
        (Some(a), Some(b)) if a < b => {
            let start = a + begin.len();
            format!(
                "{}{}{}",
                template.get(..start).unwrap_or(""),
                body,
                template.get(b..).unwrap_or("")
            )
        }
        _ => template.to_string(),
    }
}

fn product_block(p: &Product) -> String {
    format!(
        "P_NAME=\"{}\"\nP_BIN={}\nP_ID={}\nP_REPO={}\nP_PORT={}\nP_VERSION=v{}\n",
        p.name, p.bin, p.id, p.repo, p.port, p.version
    )
}

fn keys_block(p: &Product) -> String {
    let keys = p.release_keys.trim();
    if keys.is_empty() {
        "RELEASE_KEYS='\n'\n".to_string()
    } else {
        format!("RELEASE_KEYS='\n{keys}\n'\n")
    }
}

fn filled(template: &str, p: &Product) -> String {
    fill(
        &fill(template, "PRODUCT", &product_block(p)),
        "RELEASE KEYS",
        &keys_block(p),
    )
}

/// The secure installer for `p` (what `install-service` runs as root, and what
/// the product publishes as `install-service.sh`).
pub fn install_service_script(p: &Product) -> String {
    filled(INSTALL_SERVICE, p)
}

/// The secure-install CI test for `p`, whose agent endpoint is `path`
/// (published as `tests/secure-install.sh`).
pub fn secure_install_test(p: &Product, path: &str) -> String {
    fill(
        INSTALL_TEST,
        "PRODUCT",
        &format!("{}P_PATH={path}\n", product_block(p)),
    )
}

/// Every file a product publishes that is made from a base template, with its
/// path in the product's repository. `path` is the agent endpoint.
pub fn product_files(p: &Product, path: &str) -> Vec<(&'static str, String)> {
    vec![
        ("install-service.sh", install_service_script(p)),
        ("install.sh", install_script(p)),
        ("tests/secure-install.sh", secure_install_test(p, path)),
        (
            "scripts/release-key.sh",
            fill(RELEASE_KEY, "PRODUCT", &product_block(p)),
        ),
        (
            "scripts/package-manifests.sh",
            fill(MANIFESTS, "PRODUCT", &product_block(p)),
        ),
    ]
}

/// The user-level installer for `p` (published as `install.sh`).
pub fn install_script(p: &Product) -> String {
    filled(INSTALL_USER, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Product = Product {
        name: "Phygital Key Test",
        bin: "phygital-key-test",
        id: "test",
        version: "1.2.3",
        repo: "Phygital-Key/phygital-key-test",
        port: 7999,
        about: "test",
        release_keys: "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n",
    };

    #[test]
    fn installers_get_the_product_and_its_keys() {
        for s in [install_service_script(&P), install_script(&P)] {
            assert!(s.contains("P_BIN=phygital-key-test\n"), "{s}");
            assert!(s.contains("P_PORT=7999\n"));
            assert!(s.contains("P_VERSION=v1.2.3\n"));
            assert!(s.contains("RELEASE_KEYS='\n-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n'\n# END RELEASE KEYS"));
            assert_eq!(s.matches("# BEGIN PRODUCT").count(), 1);
        }
    }

    #[test]
    fn unsafe_product_values_are_refused() {
        assert!(P.is_valid());
        for bad in ["a\"b", "a$b", "a`b", "a\nb", ""] {
            assert!(!Product { name: bad, ..P }.is_valid(), "{bad:?}");
            assert!(!Product { bin: bad, ..P }.is_valid(), "{bad:?}");
        }
        assert!(!Product { repo: "a/b/c", ..P }.is_valid());
        assert!(
            !Product {
                version: "1.2",
                ..P
            }
            .is_valid()
        );
    }

    #[test]
    fn env_names() {
        assert_eq!(P.env("PORT"), "PHYGITAL_KEY_TEST_PORT");
    }
}
