# Codex-CC Channel Ingress Design

This document records the design boundary for Codex-CC's channel ingress lane.
It is intentionally transport-neutral: cc2cc, local relays, MCP servers, app
plugins, or future bridges can all use the same receive-side primitive.

## Problem

External systems sometimes need to surface inbound messages inside a running
Codex thread. The unsafe workarounds are:

- Write directly to terminal scrollback.
- Emulate keyboard input.
- Treat every inbound message as normal user input.
- Keep the message outside Codex, losing replay and resume.

These approaches blur the line between display, user intent, and model-visible
instructions.

## Core Boundary

Codex-CC adds a durable `ChannelMessage` item and a live
`thread/channel/appended` notification. A channel message is visible to the user
and persisted in rollout/thread history, but it is not automatically treated as
new user intent.

The inbound data model is:

- `id`: stable integration-provided id.
- `channel`: logical source lane.
- `sender`: human-readable sender.
- `senderKind`: `external`, `user`, `agent`, or `system`.
- `text`: full display text.
- `preview`: optional shorter display text.
- `priority`: `low`, `normal`, or `high`.
- `delivery`: `surfaceOnly` or `surfaceAndQueueNextTurn`.
- `createdAtMs`: event timestamp.
- `modelText`: optional app-server/MCP input used only for model-visible
  delivery.

## Delivery Modes

`surfaceOnly` is the default. It writes a durable item and notifies app-server
clients. It does not queue model input.

`surfaceAndQueueNextTurn` queues a developer-role metadata message for the next
model turn. The display `text` and `preview` are intentionally not copied into
model context. If the integration wants model-visible content, it must provide
`modelText`.

This preserves a clean trust boundary:

- Display text is untrusted remote content.
- `modelText` is an explicit distilled payload from the integration.
- Metadata tells the model that the content came from an installed channel
  server, not local user input.

## App-Server Path

`thread/channel_append` accepts a `ThreadChannelMessageInput`, validates the
basic fields, materializes rollout storage, emits a core `ChannelMessage` event,
flushes the rollout, and optionally queues model-visible metadata.

Core event handling projects that event back to app-server clients as
`thread/channel/appended`. Thread history replay converts the event into
`ThreadItem::ChannelMessage`, so resume and thread reads retain the item.

## MCP Logging Path

MCP servers can emit logging notifications with `logger="codex_channel"` and
`codexChannelMessage=true`. Codex parses the payload into the same
`ChannelMessageItem` type used by app-server append. This keeps MCP ingress and
app-server ingress behavior aligned:

- Surface-only messages render and persist.
- Queueing messages also wake the model when the session is idle.
- If auto-start is rejected, the queued metadata is inserted into history
  without dropping the visible channel item.

## TUI Path

Live app-server notifications and replayed thread items render as an info line:

```text
[channel] sender: preview-or-text
```

The TUI also includes channel messages in transcript fallback, resume previews,
and activity summaries so they remain inspectable outside the live scrollback.

## MCP Tool Result Presentation

Codex-CC preserves compact MCP tool-result presentation metadata. The core item
stores optional `ToolResultPresentation` alongside the raw/truncated
`CallToolResult`. TUI clients can render `summaryLines` first and fall back to
raw content rendering when the presentation is absent.

This keeps human-facing output concise while retaining raw result data for
audit, replay, and app-server clients.

## Non-Goals

Codex-CC does not implement a chat network, peer discovery, retries, transport
auth, or delivery receipts. Those belong to integrations. Codex-CC owns only
typed display, persistence, replay, and explicit model-delivery boundaries.
