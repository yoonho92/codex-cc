# Meta channel ingress design

This document describes a Codex core patch for trusted inbound channel events.
Candidate integrations include peer-to-peer bridges, external chat relays, and
other trusted local transports.

The target outcome is:

- no PTY or `cmux` input injection
- no TTY scrollback writes
- a first-class, non-primary UI surface for inbound channel messages
- durable replay across `thread/read`, resume, and fork
- a path for local integrations to append channel events using real app-server
  semantics

This is intentionally narrower than a full multi-agent orchestration system.
It focuses on "a trusted local integration needs to surface an inbound message
in the active Codex thread without pretending to be the keyboard."

## Documentation Scope

This document is the design rationale and implementation boundary record.
`docs/codex-cc/README.md` is the user-facing integration contract. The npm
distribution runbook lives in `docs/codex-cc/distribution.md`.

## Why this patch exists

Existing bridge plugins can already transport messages and even automate
handling, but the missing piece is a safe receive-side surface inside Codex
itself.

Current workarounds have hard limits:

- `tty-notify` breaks the terminal UI by writing into scrollback
- `cmux` overlays are external to Codex and are not durable thread history
- PTY input injection is unsafe and races with human typing
- `thread/inject_items` is model-visible raw history, not a channel surface

The core patch should replace those workarounds with a typed ingress lane.

## Current semantics in upstream Codex

These are the constraints the patch must respect.

### 1. `thread/inject_items` is not a channel API

`thread/inject_items` is documented as:

- append raw Responses API items to thread history
- without starting a user turn

Current code and tests show that injected items are persisted and then included
in later model input. This makes it useful for history mutation, but too broad
for a non-primary channel surface.

Relevant code:

- `codex-rs/app-server-protocol/src/protocol/common.rs`
- `codex-rs/app-server/src/codex_message_processor.rs`
- `codex-rs/app-server/tests/suite/v2/thread_inject_items.rs`

### 2. Codex already has some model-visible meta text, but it is ad hoc

Core has `ContextualUserFragment` helpers for text that is stored as `user`
content while still representing scaffolding or execution metadata.

Examples:

- environment context
- injected instructions
- subagent notifications

Relevant code:

- `codex-rs/core/src/context/contextual_user_message.rs`
- `codex-rs/core/src/context/subagent_notification.rs`
- `codex-rs/core/src/session_prefix.rs`

Important implication:

- Codex already accepts the idea that not every user-role message is primary
  user intent.
- However, this mechanism does not currently provide a first-class app-server
  or TUI channel type.

### 3. Replay and rendering are driven by typed items

Codex has a typed item pipeline:

- core `TurnItem`
- app-server `ThreadItem`
- thread history reconstruction
- TUI rendering/adaptation

Relevant code:

- `codex-rs/protocol/src/items.rs`
- `codex-rs/app-server-protocol/src/protocol/thread_history.rs`
- `codex-rs/app-server-protocol/src/protocol/v2.rs`
- `codex-rs/tui/src/app/app_server_adapter.rs`
- `codex-rs/tui/src/chatwidget.rs`

Important implication:

- a real solution should be a typed item, not only a text convention
- replay, resume, fork, and thread inspection become much easier if the new
  surface is expressed as a `TurnItem` / `ThreadItem`

### 4. TUI already distinguishes primary vs non-primary items in practice

Not every item is rendered as normal chat content.

Examples:

- `HookPrompt` exists in history but is ignored by the normal chat renderer
- `ContextCompaction` is shown as an info message, not a user/assistant turn
- command, patch, MCP, and other items already use dedicated render paths

Important implication:

- a `ChannelMessage` item can be rendered separately without forcing it into the
  standard user/assistant transcript lane

## Design goals

- Provide a trusted inbound channel lane for local integrations.
- Keep channel events attributable and replayable.
- Preserve clear separation between:
  - display plane
  - consume/model plane
- Avoid classifying inbound channel traffic as normal user intent by default.
- Make the feature reusable across multiple transports without hard-coding any
  one bridge into Codex.

## Non-goals

- Not a full networked chat server
- Not cross-account federation
- Not a replacement for subagent orchestration
- Not an invitation to let arbitrary MCP notifications mutate active CLI state
- Not automatic model visibility for every inbound message

## Proposed architecture

The patch adds a dedicated typed ingress path.

### A. New app-server request

Add a new request:

- `thread/channel_append`

Suggested params:

```json
{
  "threadId": "019d...",
  "message": {
    "id": "optional-external-id",
    "channel": "peer-inbox",
    "sender": "bridge-peer",
    "senderKind": "external",
    "text": "full inbound message text",
    "preview": "optional preview",
    "priority": "normal",
    "audience": "thread",
    "delivery": "surfaceOnly",
    "modelText": "optional distilled payload for surfaceAndQueueNextTurn"
  }
}
```

Suggested response:

```json
{
  "accepted": true,
  "itemId": "channel_..."
}
```

Why a new request instead of reusing `thread/inject_items`:

- we need narrower semantics
- we need distinct validation and audit fields
- we do not want raw Responses API behavior to be the only receive path

### B. New core item type

Add a new item to `codex-rs/protocol/src/items.rs`:

- `TurnItem::ChannelMessage(ChannelMessageItem)`

Suggested fields:

- `id`
- `channel`
- `sender`
- `sender_kind`
- `text`
- `preview`
- `priority`
- `delivery`
- `created_at_ms`

This is the durable, replayable representation.

### C. New app-server thread item

Add a matching app-server item to `codex-rs/app-server-protocol/src/protocol/v2.rs`:

- `ThreadItem::ChannelMessage`

This keeps `thread/read` and thread snapshot consumers typed and explicit.

### D. New event / live notification

Add a protocol event:

- `EventMsg::ChannelMessage(ChannelMessageEvent)`

and a server notification:

- `ServerNotification::ChannelMessageAppended`

Why both:

- `EventMsg` gives rollout durability and reconstruction
- `ServerNotification` gives live UI delivery to the current TUI/app without
  forcing a polling loop or a synthetic user turn

### E. Delivery policy

The new surface should support explicit delivery policy from day one.

Suggested enum:

- `surfaceOnly`
- `surfaceAndQueueNextTurn`

Default should be `surfaceOnly`.

Rationale:

- `surfaceOnly` solves the UI problem safely
- `surfaceAndQueueNextTurn` can later support more Claude-like behavior, but it
  should be opt-in because it affects model-visible context
- `surfaceAndQueueNextTurn` should not reuse display `text` as model input when
  `modelText` is omitted; the safe fallback is metadata-only wake context

## Display plane vs consume plane

This patch should make the split explicit.

### Display plane

What the user sees in Codex:

- a channel event
- sender identity
- preview first, then message text when no preview exists
- unread state

This should be:

- durable
- attributable
- non-primary
- safe to show while the user is typing

### Consume plane

What becomes model-visible operational input:

- only the actual message body that an agent should process
- only when explicitly promoted by policy

For a bridge plugin, this means:

- the plugin can append a visible channel event immediately
- the actual transport-side consumption can remain explicit
- `surfaceAndQueueNextTurn` with `modelText` can queue a distilled instruction
  for the next model turn
- `surfaceAndQueueNextTurn` without `modelText` queues metadata only rather than
  copying display text into model context
- hidden autodrive can later be removed or reduced once Codex has a trusted
  ingress path for follow-up work

## TUI behavior

The TUI should not render `ChannelMessage` as a normal `user` or `assistant`
chat bubble.

Recommended behavior:

- show channel items in a dedicated side panel or collapsible "Channel" section
- prefix each entry with a badge such as `[peer-inbox]` and `@sender`
- keep unread count when the section is collapsed
- allow keyboard navigation to inspect messages
- do not inject text into the composer
- do not write to terminal scrollback outside the TUI renderer

This keeps the main transcript clean while still making inbound messages visible.

## Thread history semantics

Channel items should be preserved in thread history, but they should not be
treated as primary user turns.

That means the patch should exclude them from logic that assumes normal user
intent, including at least:

- meaningful-user-turn counting
- backtrack/edit-resume selection
- message history shortcuts
- memory/summary heuristics where primary conversation is expected

The implementation does not need to solve every one of these in phase 1, but
the type must be designed so these exclusions are possible and obvious.

## Why not reuse existing item types

### Not `UserMessage`

That would make channel events look like direct user intent and would pollute
turn semantics.

### Not `AgentMessage`

That would misattribute origin as assistant output.

### Not `HookPrompt`

`HookPrompt` is specialized for hook-originated scaffolding and is currently
ignored by standard transcript rendering.

### Not raw `thread/inject_items`

That path is too broad and too model-visible for this use case.

## How transports use this

After this patch, a trusted bridge can switch to:

1. message arrives from the external transport
2. plugin calls `thread/channel_append`
3. Codex TUI shows a proper in-app channel event
4. optional follow-up action:
   - user handles it manually
   - or plugin requests a controlled consume path

This removes the need for:

- TTY writes
- `cmux` overlays as the primary UX
- PTY input injection

It also gives a real audit trail in thread history.

### MCP notification bridge

Stdio MCP servers do not automatically know the active thread id or app-server
endpoint. For MCP-hosted transports, Codex also supports a narrower ingress:

1. the MCP server emits `notifications/message`
2. `logger` is exactly `codex_channel`
3. `data.codexChannelMessage` is `true`
4. the session promotes the payload to `ChannelMessage`

Expected payload shape:

```json
{
  "codexChannelMessage": true,
  "id": "external-message-id",
  "channel": "peer-inbox",
  "sender": "bridge-peer",
  "senderKind": "agent",
  "text": "full inbound message text",
  "preview": "optional preview",
  "priority": "normal",
  "delivery": "surfaceOnly",
  "createdAtMs": 1760000000000
}
```

This keeps MCP servers transport-focused while Codex core owns rendering and
rollout persistence. Unmarked MCP logs remain ordinary logs.

## Recommended implementation phases

### Phase 1: typed surface lane

- add `ChannelMessageItem`
- add `ThreadItem::ChannelMessage`
- add `thread/channel_append`
- add `ServerNotification::ChannelMessageAppended`
- persist and replay channel items
- render them in TUI as non-primary UI

This is the minimum useful generic channel patch.

### Phase 2: explicit consume bridge

Add an opt-in promotion path so a channel event can become pending turn input
without masquerading as keyboard input.

This could be:

- `thread/channel_promote`
- or a `delivery = surfaceAndQueueNextTurn` policy

This phase should still avoid direct draft insertion.

### Phase 3: richer orchestration

Only after phases 1 and 2 work should Codex grow broader channel features such
as:

- team inboxes
- agent-to-agent mention routing
- cross-thread delivery policies

That aligns with open issues around `@mention`, multi-agent inboxes, and safe
local ingress without making them a prerequisite for any one transport.

## File areas likely touched

- `codex-rs/protocol/src/items.rs`
- `codex-rs/protocol/src/protocol.rs`
- `codex-rs/core/src/session/mod.rs`
- `codex-rs/core/src/codex_thread.rs`
- `codex-rs/app-server/src/codex_message_processor.rs`
- `codex-rs/app-server-protocol/src/protocol/common.rs`
- `codex-rs/app-server-protocol/src/protocol/v2.rs`
- `codex-rs/app-server-protocol/src/protocol/thread_history.rs`
- `codex-rs/tui/src/app/app_server_adapter.rs`
- `codex-rs/tui/src/chatwidget.rs`
- tests in app-server, protocol, and TUI suites

## Test plan

At minimum:

1. `thread/channel_append` persists a `ChannelMessage` item.
2. `thread/read` returns `ThreadItem::ChannelMessage`.
3. live TUI/app-server clients receive `ChannelMessageAppended`.
4. replay/resume/fork preserve the item.
5. `ChannelMessage` does not become a normal composer draft.
6. `ChannelMessage` is not rendered as `UserMessage` or `AgentMessage`.
7. bridge plugins can target the new API without PTY injection.

## Open questions

- Should phase 1 render full text or only preview in the channel panel?
- Should channel items live inside ordinary turns or inside a dedicated meta
  turn grouping?
- Should `surfaceAndQueueNextTurn` exist in phase 1, or wait for phase 2?
- Should the first implementation be TUI-only, or should desktop app adopt the
  same item immediately?

## Current recommendation

Start with the smallest honest patch:

- new typed channel item
- new app-server append request
- new live notification
- TUI non-primary rendering
- no automatic model visibility by default

That solves the actual channel-ingress bottleneck without repeating the
mistakes of TTY notices or keyboard emulation.
