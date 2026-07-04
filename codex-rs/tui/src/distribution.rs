use std::path::Path;

const CODEX_CC_DISTRIBUTION: &str = "codex-cc";
const CODEX_CC_NPM_ENV: &str = "CODEX_META_CHANNEL_NPM";

pub(crate) const CODEX_CC_NPM_PACKAGE: &str = "@yoonho92/codex-cc";
#[cfg(not(debug_assertions))]
pub(crate) const CODEX_CC_NPM_REGISTRY_LATEST_URL: &str =
    "https://registry.npmjs.org/@yoonho92%2Fcodex-cc/latest";
pub(crate) const CODEX_CC_RELEASE_NOTES_URL: &str = "https://github.com/yoonho92/codex-cc";

pub(crate) const CODEX_DISTRIBUTION: &str = match option_env!("CODEX_DISTRIBUTION") {
    Some(distribution) => distribution,
    None => "openai-codex",
};

pub(crate) fn is_codex_cc_build() -> bool {
    let arg0 = std::env::args_os().next();
    CODEX_DISTRIBUTION == CODEX_CC_DISTRIBUTION
        || std::env::var_os(CODEX_CC_NPM_ENV).is_some()
        || executable_name_looks_like_codex_cc(arg0.as_deref().map(Path::new))
        || executable_name_looks_like_codex_cc(std::env::current_exe().ok().as_deref())
}

#[cfg(not(debug_assertions))]
pub(crate) fn is_codex_cc_npm_distribution() -> bool {
    std::env::var_os(CODEX_CC_NPM_ENV).is_some()
}

fn executable_name_looks_like_codex_cc(path: Option<&Path>) -> bool {
    path.and_then(Path::file_stem)
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == CODEX_CC_DISTRIBUTION)
}
