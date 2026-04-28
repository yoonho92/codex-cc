# Codex-CC

Codex-CC is a Codex CLI fork that adds a typed channel ingress lane for trusted
local integrations. It keeps upstream Codex behavior intact while letting
external tools append channel messages to a running Codex thread without
pretending to type into the terminal.

The distributed command is `codex-cc`, so it can be installed next to the
official `codex` CLI.

```shell
npm install -g @yoonho92/codex-cc
codex-cc
```

Codex-CC uses split versioning. `CODEX_CC_VERSION` is this fork's npm/update
version. `CODEX_CHANNEL_BASE_VERSION` is the upstream Codex version the binary
is based on. `codex-cc --version` shows both values.

`codex-cc` does not run the official Codex updater. Local fork builds suppress
official update checks and official announcement tips. npm-installed
`codex-cc` builds check the `@yoonho92/codex-cc` package version instead and,
when an update is available, show an update command for this fork rather than
`@openai/codex`.

## What This Fork Adds

- `thread/channel_append`: typed app-server method for appending inbound channel
  events.
- `ChannelMessage`: durable thread item that survives replay, resume, fork, and
  thread reads.
- `thread/channel/appended`: live app-server notification so UI clients can
  render inbound messages without terminal scrollback writes or keyboard input
  emulation.
- Delivery control: `surfaceOnly` for display-only events, or
  `surfaceAndQueueNextTurn` when an integration intentionally wants the next
  model turn to see a distilled payload.
- MCP/app notification parsing for channel payloads using fields such as
  `text`, `channel`, `sender`, `senderKind`, `priority`, `delivery`, and
  optional `modelText`.

The goal is a clean boundary between the display plane and the model-consume
plane. Inbound channel traffic can be visible to the user without automatically
becoming primary user intent.

## Integration Quickstart

Use Codex-CC when your local integration already knows that a message should be
shown inside an active Codex thread. Codex-CC does not own your transport,
network auth, routing, retries, or peer protocol. It only provides the typed
receive-side lane.

There are two supported integration paths:

- App-server clients call `thread/channel_append` when they already have a
  `threadId`.
- MCP servers emit a logging notification with `logger="codex_channel"` and
  `codexChannelMessage=true` when they are already running inside a Codex
  session.

Minimal app-server request:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "thread/channel_append",
  "params": {
    "threadId": "019d...",
    "message": {
      "id": "external-message-id",
      "channel": "external-chat",
      "sender": "external-relay",
      "senderKind": "external",
      "text": "Full text for the channel surface.",
      "preview": "Short UI-safe preview.",
      "priority": "normal",
      "delivery": "surfaceOnly"
    }
  }
}
```

Minimal MCP channel notification payload:

```json
{
  "level": "info",
  "logger": "codex_channel",
  "data": {
    "codexChannelMessage": true,
    "id": "external-message-id",
    "channel": "external-chat",
    "sender": "external-relay",
    "senderKind": "external",
    "text": "Full text for the channel surface.",
    "preview": "Short UI-safe preview.",
    "delivery": "surfaceOnly"
  }
}
```

## Delivery Rules

- `surfaceOnly` is the safe default. It renders and persists the channel message
  without queueing model-visible work.
- `surfaceAndQueueNextTurn` may wake the model, but only `modelText` is treated
  as model-visible content.
- If `surfaceAndQueueNextTurn` is set without `modelText`, Codex queues metadata
  only. It does not copy `text` or `preview` into model context.

## Integration Checklist

- Put readable UI content in `preview` when the raw `text` is long, noisy, or
  provider-shaped.
- Keep provider tokens, raw chat ids, and secrets out of `text`, `preview`, and
  `modelText`.
- Generate stable message ids in your integration so replay and diagnostics can
  correlate events.
- Keep transport state in your integration. Codex-CC is not a delivery receipt,
  retry queue, or external chat protocol.

## Build And Distribution

Build a versioned binary and package it as `codex-cc`:

```shell
CODEX_CC_VERSION=0.1.0 \
CODEX_CHANNEL_BASE_VERSION=0.124.0 \
scripts/build-meta-channel-codex.sh
scripts/package-meta-channel-npm.sh
```

The build script injects the Codex-CC package version and the upstream Codex
base version at compile time. The TUI keeps showing the base Codex version for
upstream compatibility, while npm update checks compare Codex-CC package
versions. It also marks the binary as the `codex-cc` distribution, so official
Codex update prompts and remote announcement tips are not shown for local fork
builds.

## Documentation Map

- This file is the user-facing integration contract for Codex-CC.
- [Distribution](./distribution.md) covers build profiles, local tarball
  testing, npm package layout, and publishing.
- [Ingress Design](./ingress-design.md) records design rationale, protocol
  boundaries, and phased implementation notes.

If this file and a deeper design document disagree, treat this file as the
user-facing contract and update the design document or implementation notes.
