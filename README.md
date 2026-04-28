<p align="center"><strong>Codex-CC Fork</strong></p>
<p align="center">
  A Codex CLI fork with a typed channel ingress lane for trusted local integrations.
</p>
<p align="center"><code>npm i -g @yoonho92/codex-cc</code><br />then run <code>codex-cc</code></p>

---

## Fork Notice

This repository is a fork of OpenAI Codex CLI. The upstream README is preserved
below so existing Codex users can still find the normal installation,
authentication, and contribution documentation.

The fork-specific feature is Codex-CC: a transport-neutral channel ingress
surface for local integrations. It adds typed channel messages without making
any external bridge, chat provider, or peer protocol part of Codex core.

## Codex-CC Overview

Codex-CC keeps upstream Codex behavior intact while adding one receive-side
capability: trusted local tools can surface inbound messages inside a running
Codex thread without writing into terminal scrollback or emulating keyboard
input.

This is intended for integrations such as peer-agent bridges, external chat
relays, local daemons, or workflow tools that already own their transport and
need Codex to display, persist, replay, and optionally queue a distilled channel
message.

### What This Fork Adds

- `thread/channel_append`: typed app-server method for appending inbound channel
  events.
- `ChannelMessage`: durable thread item that survives replay, resume, fork, and
  thread reads.
- `thread/channel/appended`: live app-server notification so UI clients can
  render inbound messages immediately.
- `surfaceOnly`: default display-only delivery mode.
- `surfaceAndQueueNextTurn`: explicit opt-in delivery mode for model-visible
  work.
- `modelText`: optional distilled model-visible payload. Display `text` and
  `preview` are not copied into model context when `modelText` is omitted.
- `codex-cc`: separate executable name, so this fork can be installed beside
  the official `codex` CLI.

### Quickstart

```shell
npm install -g @yoonho92/codex-cc
codex-cc
```

For local builds:

```shell
scripts/build-meta-channel-codex.sh
scripts/package-meta-channel-npm.sh
```

### Integration Shape

App-server clients can append a channel message when they already have a
`threadId`:

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

MCP servers can emit the same kind of channel event with
`logger="codex_channel"` and `codexChannelMessage=true`. See the Codex-CC docs
for the full payload contract and safety rules.

### Project Boundaries

Codex-CC is not a chat network, transport daemon, retry queue, or peer protocol.
Those belong in separate integrations. Codex-CC owns only the core channel lane:
typed display, durable replay, and explicit optional model delivery.

Start here for Codex-CC:

- [Codex-CC documentation](./docs/codex-cc/README.md)
- [Distribution and npm packaging](./docs/codex-cc/distribution.md)
- [Channel ingress design notes](./docs/codex-cc/ingress-design.md)

---

<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install --cask codex</code></p>
<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

## Quickstart

### Installing and running Codex CLI

Install globally with your preferred package manager:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Business, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Codex-CC fork documentation**](./docs/codex-cc/README.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
