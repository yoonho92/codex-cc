/// The current Codex CLI version as embedded at compile time.
///
/// Forked builds can set this at build time so the app reports the upstream
/// Codex release it is based on instead of the workspace's local-dev version.
pub const CODEX_CLI_VERSION: &str = match option_env!("CODEX_CLI_VERSION_OVERRIDE") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};
