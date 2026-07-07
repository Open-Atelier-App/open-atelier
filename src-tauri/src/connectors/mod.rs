//! Opt-in third-party connectors (see Settings > Connectors). Each
//! connector is off by default and requires the user to paste their own
//! credential — enabling one means the assistant can send data (file
//! paths, content) to that external service, which is why this is kept as
//! a separate, clearly-flagged opt-in rather than bundled into the
//! always-on local trigger protocol.
pub mod github;
pub mod github_oauth;
pub mod notion;
pub mod slack;
pub mod google_drive;
pub mod google_oauth;
