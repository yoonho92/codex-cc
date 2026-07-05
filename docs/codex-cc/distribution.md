# Codex-CC npm Distribution

Codex-CC is a Codex CLI fork for trusted local channel integrations. It keeps
the normal Codex CLI behavior, but adds a typed receive-side lane that external
tools can use to surface messages inside an active Codex thread.

Repository: <https://github.com/yoonho92/codex-cc>

The distributed command is `codex-cc`. It intentionally does not overwrite the
official `codex` command.

Codex-CC uses split versioning:

- `CODEX_CC_VERSION`: the Codex-CC npm package and update-check version.
- `CODEX_CLI_VERSION_OVERRIDE`: the upstream Codex version this fork is based
  on.
- `CODEX_CC_DISPLAY_VERSION`: the string shown by `codex-cc --version`, usually
  `<base> (codex-cc <package>)`.

## Install

After publishing, install globally:

```bash
npm install -g @yoonho92/codex-cc
codex-cc --version
codex-cc
```

For local tarball testing:

```bash
npm install -g dist/npm/meta-channel/yoonho92-codex-cc-<version>.tgz
codex-cc --version
codex-cc
```

## Build

Build a versioned binary first:

```bash
CODEX_CC_VERSION=0.1.0 \
CODEX_CHANNEL_BASE_VERSION=0.142.5 \
scripts/build-meta-channel-codex.sh
```

The build script detects the installed upstream Codex version when
`CODEX_CHANNEL_BASE_VERSION` is not set. It injects:

- `CODEX_CLI_VERSION_OVERRIDE` for the upstream base version.
- `CODEX_CC_VERSION` for the fork package version.
- `CODEX_CC_DISPLAY_VERSION` for `codex-cc --version`.
- `CODEX_DISTRIBUTION=codex-cc` to suppress official update prompts and
  announcement tips in local fork builds.

By default the build uses Cargo's `release` profile and writes artifacts under
`../codex-target`.

If a future upstream base defines a custom distribution profile, select it
explicitly:

```bash
CODEX_CHANNEL_BUILD_PROFILE=dist scripts/build-meta-channel-codex.sh
```

For fast local validation only:

```bash
CODEX_CHANNEL_BUILD_PROFILE=debug scripts/build-meta-channel-codex.sh
```

The packaging script expects a `dist` or `release` binary by default. Set
`CODEX_CHANNEL_ALLOW_DEBUG_BINARY=1` only for local packaging tests.

## Package

Create npm tarballs:

```bash
scripts/package-meta-channel-npm.sh
```

Defaults:

- npm package: `@yoonho92/codex-cc`
- executable: `codex-cc`
- output directory: `dist/npm/meta-channel`

The generated npm wrapper sets `CODEX_META_CHANNEL_NPM=1`. Codex-CC uses that
runtime marker to check the npm registry for `@yoonho92/codex-cc` updates and
to display `npm install -g @yoonho92/codex-cc` as the update command. The
comparison is `CODEX_CC_VERSION` vs the npm package version; it does not compare
against the upstream Codex base version and does not point npm-installed
Codex-CC users at `@openai/codex`.

Codex-CC stores this update cache in `codex-cc-version.json`, separate from
upstream Codex's `version.json`, so stale upstream release checks cannot produce
mixed prompts such as `0.1.0 -> 0.142.5`.

Automation launchers can set `CODEX_DISABLE_UPDATE_PROMPT=1` to prevent the
interactive update modal from blocking startup. This is intended for
launcher-owned participant sessions, not for changing the default interactive
user experience.

## Publish

Publish the platform payload first, then the wrapper package:

```bash
npm publish --access public dist/npm/meta-channel/yoonho92-codex-cc-darwin-arm64-<version>.tgz
npm publish --access public dist/npm/meta-channel/yoonho92-codex-cc-<version>.tgz
```

Build and publish each target platform from the matching OS/architecture. The
wrapper command remains `codex-cc`.

## Project Boundaries

Codex-CC is not a new chat network and not a transport-specific build. It
provides the core receive-side lane. External chat relays, peer-agent bridges,
local daemons, or other plugins remain separate projects that can choose to use
this lane.

The safe default is:

- Transport owns network/file/API details.
- Codex-CC owns typed display, replay, and optional model delivery.
- The user-facing command remains separate from official Codex: `codex-cc`.
