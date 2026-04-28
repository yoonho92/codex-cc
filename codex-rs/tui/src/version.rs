/// The upstream Codex CLI version this build is based on.
///
/// Forked builds can set this at build time so the app reports the upstream
/// Codex release it is based on instead of the workspace's local-dev version.
pub const CODEX_BASE_VERSION: &str = match option_env!("CODEX_CLI_VERSION_OVERRIDE") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// The Codex-CC distribution/package version.
///
/// This is intentionally separate from the upstream Codex base version because
/// Codex-CC can release packaging/channel fixes without rebasing upstream.
pub const CODEX_CC_VERSION: &str = match option_env!("CODEX_CC_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// Backwards-compatible name used by existing TUI surfaces for the base Codex
/// version.
pub const CODEX_CLI_VERSION: &str = CODEX_BASE_VERSION;
