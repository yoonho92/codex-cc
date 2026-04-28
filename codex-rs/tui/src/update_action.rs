#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
#[cfg(any(not(debug_assertions), test))]
use codex_install_context::StandalonePlatform;

/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update the Codex-CC fork via `npm install -g @yoonho92/codex-cc`.
    CodexCcNpmGlobalLatest,
    /// Update via `npm install -g @openai/codex@latest`.
    NpmGlobalLatest,
    /// Update via `bun install -g @openai/codex@latest`.
    BunGlobalLatest,
    /// Update via `brew upgrade codex`.
    BrewUpgrade,
    /// Update via `curl -fsSL https://chatgpt.com/codex/install.sh | sh`.
    StandaloneUnix,
    /// Update via `irm https://chatgpt.com/codex/install.ps1|iex`.
    StandaloneWindows,
}

impl UpdateAction {
    pub fn current_version(self) -> &'static str {
        match self {
            UpdateAction::CodexCcNpmGlobalLatest => crate::version::CODEX_CC_VERSION,
            UpdateAction::NpmGlobalLatest
            | UpdateAction::BunGlobalLatest
            | UpdateAction::BrewUpgrade
            | UpdateAction::StandaloneUnix
            | UpdateAction::StandaloneWindows => crate::version::CODEX_CLI_VERSION,
        }
    }

    #[cfg(any(not(debug_assertions), test))]
    pub(crate) fn from_install_context(context: &InstallContext) -> Option<Self> {
        match context {
            InstallContext::Npm => Some(UpdateAction::NpmGlobalLatest),
            InstallContext::Bun => Some(UpdateAction::BunGlobalLatest),
            InstallContext::Brew => Some(UpdateAction::BrewUpgrade),
            InstallContext::Standalone { platform, .. } => Some(match platform {
                StandalonePlatform::Unix => UpdateAction::StandaloneUnix,
                StandalonePlatform::Windows => UpdateAction::StandaloneWindows,
            }),
            InstallContext::Other => None,
        }
    }

    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::CodexCcNpmGlobalLatest => (
                "npm",
                &["install", "-g", crate::distribution::CODEX_CC_NPM_PACKAGE],
            ),
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "--cask", "codex"]),
            UpdateAction::StandaloneUnix => (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"],
            ),
            UpdateAction::StandaloneWindows => (
                "powershell",
                &["-c", "irm https://chatgpt.com/codex/install.ps1|iex"],
            ),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }

    pub fn release_notes_url(self) -> &'static str {
        match self {
            UpdateAction::CodexCcNpmGlobalLatest => crate::distribution::CODEX_CC_RELEASE_NOTES_URL,
            UpdateAction::NpmGlobalLatest
            | UpdateAction::BunGlobalLatest
            | UpdateAction::BrewUpgrade
            | UpdateAction::StandaloneUnix
            | UpdateAction::StandaloneWindows => "https://github.com/openai/codex/releases/latest",
        }
    }
}

#[cfg(not(debug_assertions))]
pub(crate) fn get_update_action() -> Option<UpdateAction> {
    if crate::distribution::is_codex_cc_npm_distribution() {
        return Some(UpdateAction::CodexCcNpmGlobalLatest);
    }
    if crate::distribution::is_codex_cc_build() {
        return None;
    }
    UpdateAction::from_install_context(InstallContext::current())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn maps_install_context_to_update_action() {
        let native_release_dir = PathBuf::from("/tmp/native-release");

        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Other),
            None
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Npm),
            Some(UpdateAction::NpmGlobalLatest)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Bun),
            Some(UpdateAction::BunGlobalLatest)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Brew),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Standalone {
                platform: StandalonePlatform::Unix,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneUnix)
        );
        assert_eq!(
            UpdateAction::from_install_context(&InstallContext::Standalone {
                platform: StandalonePlatform::Windows,
                release_dir: native_release_dir.clone(),
                resources_dir: Some(native_release_dir.join("codex-resources")),
            }),
            Some(UpdateAction::StandaloneWindows)
        );
    }

    #[test]
    fn codex_cc_update_command_targets_the_fork_package() {
        assert_eq!(
            UpdateAction::CodexCcNpmGlobalLatest.command_args(),
            ("npm", &["install", "-g", "@yoonho92/codex-cc"][..])
        );
        assert_eq!(
            UpdateAction::CodexCcNpmGlobalLatest.current_version(),
            crate::version::CODEX_CC_VERSION
        );
        assert_eq!(
            UpdateAction::CodexCcNpmGlobalLatest.release_notes_url(),
            "https://github.com/yoonho92/codex"
        );
    }

    #[test]
    fn standalone_update_commands_rerun_latest_installer() {
        assert_eq!(
            UpdateAction::StandaloneUnix.command_args(),
            (
                "sh",
                &["-c", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"][..],
            )
        );
        assert_eq!(
            UpdateAction::StandaloneWindows.command_args(),
            (
                "powershell",
                &["-c", "irm https://chatgpt.com/codex/install.ps1|iex"][..],
            )
        );
    }
}
