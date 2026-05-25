# Qsh ⚡

_qsh = "question shell." `?` was already the universal symbol for "I don't know. Figure it out." Now it actually does._

Yet another natural-language shell tool. Here's the thing that makes this one stick:

![qsh demo](demo.gif)

🔁 A bare `?` within 10 minutes of a failed command replays the original intent plus the last 3 attempts and their stderr. The model fixes what actually broke instead of re-cycling through approaches it already tried. None of the other AI-shell tools I tried did this. It's the difference between a screenshot demo and something you reach for daily.

`?` is fast mode. `??` is smart mode (**reasoning on**). Works in zsh, bash, fish. Backed by Gemini, OpenAI, Claude, or local Ollama. Every command waits for `y`. The rest of this README is taste-level decisions: stdout-is-the-command discipline, cache-on-edit, single-request `--alts N`, cross-distro detection, and a 🛡️ safety model that refuses `rm -rf /` but lets `rm -rf /tmp/build` through because you took responsibility by naming it.

---

## Features

- Natural-language shell commands through `?` and smarter `??` mode.
- Confirmation before execution, with edit and refine actions at the prompt.
- Failure-aware retry: a bare `?` after a failed generated command includes the recent stderr so the next candidate can fix the actual error.
- Context-aware suggestions from the current project, piped stdin, and explicit `./file` references.
- Distro-aware command generation that adapts to Linux, BSD, and macOS userlands.
- `--alts` can return several distinct command candidates in one request when you want options.
- `-e/--explain` adds a short `# why:` note for learning or quick review.
- Local caching speeds up repeated queries and keeps edited commands as the remembered answer.
- Hosted providers, local Ollama, and Claude/Codex CLI backends.

## Install

Install the latest release to `~/.local/bin/qsh`:

```sh
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh
```

Add shell integration:

```sh
eval "$(qsh init zsh)"      # zsh
eval "$(qsh init bash)"     # bash
qsh init fish | source      # fish
```

For a permanent setup, put the matching line in your shell rc file.

Build from source:

```sh
git clone https://github.com/abdulrahman1s/qsh.git
cd qsh
cargo build --release
install -m 0755 target/release/qsh ~/.local/bin/qsh
```

<details>
<summary>NixOS users</summary>

Use the flake instead of a manual install:

```sh
nix run github:abdulrahman1s/qsh#qsh -- --help
nix profile install github:abdulrahman1s/qsh#qsh
```

For the NixOS module and prebuilt-release option, see [DOCS.md](DOCS.md#nix-flakes).

</details>

## Provider Setup

Set at least one hosted-provider API key, a local Ollama model, or a logged-in CLI backend:

```sh
export GEMINI_API_KEY="..."
export ANTHROPIC_API_KEY="..."
export OPENAI_API_KEY="..."
export OLLAMA_MODEL="qwen3:8b"
```

CLI backends:

```sh
claude /login
qsh config set providers.claude.backend cli

codex login
qsh config set providers.openai.backend cli
```

Auto-detect order is `gemini > claude > openai > ollama > claude CLI > codex CLI`. Pin a provider with `QSH_PROVIDER`, or use `qsh config`.

## Usage

```sh
? find rust files modified this week
?? rebase my last 5 commits onto main and drop the wip ones
git status | ? what should I do
? ./Cargo.toml bump tokio to the latest minor
? -e show all open ports with the owning process
? --alts 4 dedupe lines from this file keeping the most recent
```

Prompt actions:

| Key | Action |
| --- | ------ |
| `y` | Run the command |
| `n` | Decline |
| `e` | Edit before running |
| `r` | Refine with a follow-up instruction |
| `?` | Show prompt help |

## Documentation

See [DOCS.md](DOCS.md) for installation variants, shell behavior, configuration, provider details, context handling, retries, caching, privacy/security notes, and development workflows.

## Security and Privacy

`qsh` is a command generator, not a permission boundary. Every command is shown before execution, and pressing Enter declines by default. Read the command before pressing `y`.

Prompts, piped stdin, and file context are sent to the provider or backend you select. Use local Ollama when you want requests to stay on your configured local endpoint, and avoid piping secrets to hosted providers or CLI backends.

API keys are sent in provider headers, not URLs or stdout. Debug output redacts keys, but it can still include your prompt and any stdin/file context.

## Disclaimer

This tool generates and runs shell commands produced by a language model. Models hallucinate, misread context, and can produce dangerous commands. The safety hard-stops and confirmation prompt are defense in depth, not a guarantee.

Anything you press `y` on runs with your shell's full privileges. Read the command on the line. If you do not understand it, do not run it. Use at your own risk.

## License

MIT.
