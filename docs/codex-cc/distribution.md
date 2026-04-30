# Codex-CC npm Distribution

Codex-CC is a Codex CLI fork for trusted local channel integrations. It keeps
the normal Codex CLI behavior, but adds a typed receive-side lane that external
tools can use to surface messages inside an active Codex thread.

Repository: <https://github.com/yoonho92/codex-cc>

The distributed command is `codex-cc`. It intentionally does not overwrite the
official `codex` command.

Codex-CC uses split versioning:

- `CODEX_CC_VERSION`: the Codex-CC npm package and update-check version.
- `CODEX_CHANNEL_BASE_VERSION`: the upstream Codex version this fork is based
  on.

This lets Codex-CC publish channel/package fixes without pretending the
underlying upstream Codex version changed, and lets upstream rebases be tracked
without forcing the Codex-CC product version to mirror upstream.

## Documentation Scope

This document is the packaging and distribution runbook. For the shortest
integration path, start with `docs/codex-cc/README.md`. For protocol rationale
and implementation boundaries, see `docs/codex-cc/ingress-design.md`.

## Why It Exists

Bridge plugins can already move messages between agents or services. The hard
part is delivering an inbound message into Codex safely.

The old workarounds are not good enough:

- Writing to terminal scrollback can break the TUI.
- Emulating keyboard input races with the human operator and can become remote
  control.
- Treating every inbound message as normal user text pollutes the main
  conversation lane.
- Keeping the message outside Codex makes replay, resume, and debugging weak.

Codex-CC adds a real core surface for this: a typed channel message that can be
rendered, replayed, audited, and optionally queued for model consumption.

## Core Interfaces

Codex-CC exposes the channel lane through the app-server protocol.

### Append A Channel Message

Use `thread/channel_append` to append a trusted inbound message:

```json
{
  "threadId": "019d...",
  "message": {
    "id": "external-message-id",
    "channel": "peer-inbox",
    "sender": "codex-peer",
    "senderKind": "agent",
    "text": "Full message shown in the channel surface.",
    "preview": "Short optional preview.",
    "priority": "normal",
    "delivery": "surfaceOnly",
    "modelText": "Optional distilled model-visible payload.",
    "createdAtMs": 1777300000000
  }
}
```

The response contains:

```json
{
  "accepted": true,
  "itemId": "external-message-id"
}
```

### Observe Live Delivery

Clients can subscribe to `thread/channel/appended`. The notification carries the
typed `ChannelMessage` thread item, so UI clients do not need to scrape terminal
output or parse human-formatted text.

### Persisted Thread Item

The durable item shape is:

```json
{
  "type": "channelMessage",
  "id": "external-message-id",
  "channel": "peer-inbox",
  "sender": "codex-peer",
  "senderKind": "agent",
  "text": "Full message shown in the channel surface.",
  "preview": "Short optional preview.",
  "priority": "normal",
  "delivery": "surfaceOnly",
  "createdAtMs": 1777300000000
}
```

This is available through thread history and replay paths.

### MCP/App Notification Payloads

MCP or app integrations can emit channel-style payloads with these fields:

- `text`: required display text.
- `channel`: optional channel name, defaults to `mcp`.
- `sender` or `from`: optional sender name, defaults to the server name.
- `senderKind`: `external`, `user`, `agent`, or `system`.
- `priority`: `low`, `normal`, or `high`.
- `delivery`: `surfaceOnly` or `surfaceAndQueueNextTurn`.
- `modelText`: optional distilled text to queue for the next model turn when
  delivery is `surfaceAndQueueNextTurn`.

## Delivery Modes

`surfaceOnly` is the default. The message is visible and durable, but it is not
automatically treated as new user intent.

`surfaceAndQueueNextTurn` is explicit opt-in model delivery. Use it only when
the integration has already decided what the model should see. Prefer a concise
`modelText` over copying an entire external message verbatim.

If `surfaceAndQueueNextTurn` is set without `modelText`, Codex queues only a
metadata-only developer message. The display `text` and `preview` are not reused
as model-visible content. This prevents a UI surface message from accidentally
becoming remote instructions.

This separation is the main point of the core patch: display and model context
are related, but not the same thing.

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
CODEX_CHANNEL_BASE_VERSION=0.124.0 \
scripts/build-meta-channel-codex.sh
```

The build script detects the installed upstream Codex version, for example
`codex-cli 0.124.0`, and injects it with `CODEX_CLI_VERSION_OVERRIDE`. This
keeps the TUI header from showing the workspace development version `0.0.0`.
It also injects `CODEX_CC_VERSION`, and `codex-cc --version` shows both values
as `codex-cli <base> (codex-cc <package>)`.

You can pin the base version explicitly:

```bash
CODEX_CHANNEL_BASE_VERSION=0.124.0 scripts/build-meta-channel-codex.sh
```

You should bump `CODEX_CC_VERSION` for every npm release:

```bash
CODEX_CC_VERSION=0.1.1 scripts/build-meta-channel-codex.sh
```

By default the build uses the custom Cargo `dist` profile. It keeps release
optimizations and symbol stripping, but uses thin LTO and more codegen units so
local distribution builds finish much faster than upstream's smallest-binary
release profile. The target directory is `../codex-target` so the repository is
not filled with build artifacts.

For the smallest upstream-style binary:

```bash
CODEX_CHANNEL_BUILD_PROFILE=release scripts/build-meta-channel-codex.sh
```

For fast local validation only, a debug build is available:

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

The package README is generated into the npm package root. npm and registry
pages will therefore show the Codex-CC install instructions, purpose, and core
interfaces rather than only the upstream Codex README.

The generated npm wrapper sets `CODEX_META_CHANNEL_NPM=1`. Codex-CC uses that
runtime marker to check the npm registry for `@yoonho92/codex-cc` updates and
to display `npm install -g @yoonho92/codex-cc` as the update command. The
comparison is `CODEX_CC_VERSION` vs the npm package version; it does not compare
against the upstream Codex base version and does not point npm-installed
Codex-CC users at `@openai/codex`. Codex-CC stores this update cache in
`codex-cc-version.json`, separate from upstream Codex's `version.json`, so stale
upstream release checks cannot produce mixed prompts such as
`0.1.0 -> 0.125.0`.

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
- Local fork builds suppress official Codex update checks and OpenAI remote
  announcement tips. npm builds use the Codex-CC package update path instead.
