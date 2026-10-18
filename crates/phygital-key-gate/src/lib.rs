//! # phygital-key-gate
//!
//! The engine every Phygital Key product shares: the approval page, enrollment and
//! the signed key history, one-time and standing approvals, revocations, receipts,
//! the hash-chained audit log, the secure install, signed automatic updates and
//! the shared commands (`serve`, `enroll`, `keys`, `verify`, ...).
//!
//! A product adds only the agent protocol it speaks, as an [`Adapter`], and calls
//! [`run`] from its `main`:
//!
//! ```ignore
//! fn main() {
//!     phygital_key_gate::run(&PRODUCT, MyAdapter)
//! }
//! ```
//!
//! The gate can never approve: only a person's key signs, on the approval page.

#![forbid(unsafe_code)]

mod audit;
mod cli;
mod demo;
mod product;
mod seen;
pub mod state;
mod update;
pub mod util;
mod web;

pub use cli::run;
pub use product::{Adapter, Product, Task, install_script, install_service_script, product};
pub use state::{Gate, Settings, Shared, Status, View};
