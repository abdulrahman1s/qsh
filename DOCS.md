# Qsh Documentation

`qsh` is a zsh-first shell-command generator with bash and fish wrappers. It prints generated commands to stdout and keeps all UI on stderr so the shell wrapper can safely evaluate only the accepted command.

## Contents

- [Install](#install)
- [Provider setup](#provider-setup)
- [Shell integration](#shell-integration)
- [Basic usage](#basic-usage)
- [Context](#context)
- [Configuration](#configuration)
- [Advanced workflows](#advanced-workflows)
- [Reference](#reference)
- [Safety and privacy](#safety-and-privacy)
- [How it works](#how-it-works)
- [Development](#development)

## Install

### Install script

Download the latest prebuilt GitHub Release binary and install it to `~/.local/bin/qsh`:

```sh
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh
```

The script detects the host release target, downloads `qsh-<version>-<target>.tar.gz`, and verifies the `.sha256` asset when a local SHA-256 tool is available. It is user-only by default and does not use `sudo`, `doas`, or root permissions.

After install, the script detects your shell from `$SHELL` and asks via `/dev/tty` (so it still works under `curl … | sh`):

```text
==> add qsh shell integration to /home/you/.zshrc? [Y/n]
```

Pressing Enter accepts the default and appends the `qsh init` line plus a `PATH` export if needed. The append is idempotent — re-running the installer won't duplicate entries.

If `/dev/tty` is unavailable (CI, non-interactive shell), the rc file is left alone and the installer prints a hint instead. Pass `--yes` to make it append automatically in that case.

Skip the prompt with explicit flags:

```sh
# auto-accept the prompt
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh -s -- --yes

# never touch any rc file
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh -s -- --no-modify-rc

# pick a specific shell instead of auto-detecting
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh -s -- --zshrc
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh -s -- --bashrc
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh -s -- --fishrc
```

`QSH_YES=1` and `QSH_NO_MODIFY_RC=1` work as environment-variable equivalents of `--yes` and `--no-modify-rc`.

### From source

```sh
git clone https://github.com/abdulrahman1s/qsh.git
cd qsh
cargo build --release
mkdir -p ~/.local/bin
install -m 0755 target/release/qsh ~/.local/bin/qsh
```

Then add the wrapper for your shell:

```zsh
echo 'eval "$(qsh init zsh)"' >> ~/.zshrc
exec zsh
```

```bash
echo 'eval "$(qsh init bash)"' >> ~/.bashrc
exec bash
```

```fish
mkdir -p ~/.config/fish
echo 'qsh init fish | source' >> ~/.config/fish/config.fish
exec fish
```

New shells will have `qsh`, `?`, and `??` available. Fish installs `?` and `??` as functions.

Quote glob patterns in fish queries when you want the model to see them literally:

```fish
? find "*.rs"
```

### Nix flakes

From this checkout:

```sh
nix run .#qsh -- --help
nix profile install .#qsh
```

In a NixOS flake, add this repo as an input and enable the module:

```nix
{
  inputs.qsh.url = "github:abdulrahman1s/qsh";
  inputs.qsh.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { nixpkgs, qsh, ... }: {
    nixosConfigurations.host = nixpkgs.lib.nixosSystem {
      modules = [
        qsh.nixosModules.default
        {
          programs.qsh = {
            enable = true;
            enableZshIntegration = true;
            # enableBashIntegration = true;
            # enableFishIntegration = true;
          };
        }
      ];
    };
  };
}
```

By default the module builds from source. To install a GitHub Release binary instead, enable `prebuilt` and pin the release tarball hash:

```nix
programs.qsh = {
  enable = true;
  prebuilt = {
    enable = true;
    version = "0.2.0";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
  };
};
```

Get the hash with:

```sh
nix store prefetch-file --json \
  https://github.com/abdulrahman1s/qsh/releases/download/v0.2.0/qsh-v0.2.0-x86_64-unknown-linux-gnu.tar.gz
```

## Provider Setup

Set at least one hosted-provider API key, an Ollama model, or a logged-in CLI backend in your shell environment:

```sh
export GEMINI_API_KEY="..."        # https://aistudio.google.com/apikey
export ANTHROPIC_API_KEY="..."     # https://console.anthropic.com/settings/keys
export OPENAI_API_KEY="..."        # https://platform.openai.com/api-keys
export OLLAMA_MODEL="qwen3:8b"     # optional local provider default
```

In fish, use `set -gx NAME "..."` instead of `export NAME="..."`.

Auto-detect order is:

```text
gemini > claude > openai > ollama > claude CLI > codex CLI
```

If multiple providers are available, pin your default:

```sh
export QSH_PROVIDER=claude
```

### Ollama

Ollama uses `http://127.0.0.1:11434` by default. Override it with either variable:

```sh
export OLLAMA_HOST="http://127.0.0.1:11434"
export OLLAMA_BASE_URL="http://127.0.0.1:11434/v1"
```

Accepted forms are `host:port`, `http://host:port`, or a base URL ending in `/v1`.

### CLI Backends

Claude and OpenAI can run through their local CLIs instead of direct HTTP APIs. API keys win during auto-detect, so choose CLI explicitly when you want it:

```sh
claude /login
qsh config set providers.claude.backend cli

codex login
qsh config set providers.openai.backend cli
```

For per-project overrides, add `backend = "cli"` to `qsh.toml`. The CLI backends translate mode → reasoning effort: `fast` passes `low` and `smart` passes `high` (via `claude --effort` and `codex -c model_reasoning_effort=...`). They still ignore the `tokens.smart` budget — those API knobs aren't exposed by the CLIs.

## Shell Integration

`qsh generate` writes only the final accepted command to stdout. The shell wrapper printed by `qsh init <shell>` evaluates that stdout in the current shell, captures stderr from the executed command, and records failures for retry.

Generate wrapper code:

```sh
qsh init zsh
qsh init bash
qsh init fish
```

Smoke-test syntax:

```sh
target/release/qsh init zsh
zsh -n <(target/release/qsh init zsh)

target/release/qsh init bash
bash -n <(target/release/qsh init bash)

target/release/qsh init fish | fish -n
```

## Basic Usage

```sh
? find rust files modified this week
?? rebase my last 5 commits onto main and drop the wip ones
? -c port-forward 8080 to my staging cluster
? -o -m gpt-5.4 convert all png files in this dir to webp
? -l -m qwen3:8b summarize disk usage here
```

`?` is fast mode. `??` is smart mode with extended thinking where the selected provider supports it. Smart mode is slower and usually costs more, but is better for harder shell design tasks.

### Prompt Actions

```text
Run?  [Y]es  [N]o  [E]dit  [R]efine  [?]
```

| Key | Action |
| --- | ------ |
| `y` | Run the command |
| `n` | Decline. Plain Enter also declines |
| `e` | Edit the command before running |
| `r` | Refine with a follow-up directive |
| `?` | Show inline prompt help |

Every generated command requires confirmation before execution.

## Context

### Stdin Context

Anything piped in is included as labelled context for the model:

```sh
git status | ? what should I do
git diff --stat | ? what does this PR look like
git log --oneline -20 | ? summarize what changed recently

cat err.log | ? why is this failing
cargo build 2>&1 | ? explain this rust error
journalctl -u nginx -n 50 --no-pager | ? what's wrong with nginx

ps aux --sort=-%mem | head -20 | ? which of these should I kill
df -h | ? which mount is almost full
docker ps -a | ? clean up the stopped containers
kubectl get pods -A | ? which pod is unhealthy and why
```

Stdin is capped at 32 KB. The first 32 KB is used by default. If you need the tail, pipe through `tail` first:

```sh
tail -c 32k server.log | ? what's the last error here
```

You can combine stdin context with an explicit task:

```sh
git status | ? -e prepare a clean-up commit
```

### File Context

Any argument starting with `./` and pointing at a readable file is loaded inline as labelled context. Multiple file refs are allowed, with a shared 32 KB cap.

```sh
? ./Cargo.toml bump tokio to the latest minor
? ./config.yaml ./schema.json check these match
? ./errors.log why is this failing
```

A `./token` that does not resolve to a file is treated as task text. Directories are ignored.

Line slicing:

| Syntax | What it sends |
| ------ | ------------- |
| `./file.log:50` | First 50 lines |
| `./file.log:-50` | Last 50 lines |
| `./file.log:120-180` | Lines 120 through 180 inclusive |

```sh
? ./build.log:-100 why is this failing
? ./schema.sql:1-40 ./schema.sql:-20 summarize the head and tail
```

Slices still count against the 32 KB total budget. If a slice would overflow it, the read is truncated and the info line says so. A path that literally ends in `:N` is read whole when it exists on disk; slice parsing only applies when the bare path does not resolve.

File context works with stdin and explain mode:

```sh
cat manifest.json | ? ./schema.json does the stdin match the schema
? -e ./Dockerfile what does ARG vs ENV do here
```

### Cwd Context

Unless disabled, `qsh` probes the current directory for:

- Git branch.
- Language manifests such as `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `deno.json`, and `mix.exs`.
- Build tooling such as `flake.nix`, `Makefile`, `justfile`, `Dockerfile`, and `compose.yaml`.

This lets tasks such as "run the tests" or "format this project" pick the right local tool. Disable it per call with `--no-context` or per environment with `QSH_NO_CONTEXT=1`.

## Configuration

Configuration can come from CLI flags, environment variables, `qsh.toml`, and the global config file.

### Per-Project `qsh.toml`

Drop a `qsh.toml` in any directory, or an ancestor of it, to set defaults for that tree. The closest `qsh.toml` wins.

```toml
# qsh.toml
provider = "claude"
backend  = "cli"
mode     = "smart"
model    = "claude-sonnet-4-6"

# Optional: pick a different model per mode. When set, `models.fast` /
# `models.smart` override `model` based on the resolved mode.
[models]
fast  = "claude-haiku-4-5"
smart = "claude-opus-4-7"

prompt = """
This codebase uses Bun, not Node. Prefer bun over npm/pnpm.
Tests run with bun test; build is bun run build.
Always prefer ripgrep over grep, fd over find.
"""
```

Recognized top-level keys are `provider`, `backend`, `mode`, `model`, and `prompt`. The `[models]` table accepts `fast` / `smart` for per-mode model overrides. The `prompt` value is free-form project guidance appended to the system prompt.

Per TOML rules, top-level keys (`provider`, `mode`, `model`, `prompt`, ...) must appear **before** any `[table]` header — otherwise they'll be parsed as members of that table.

Precedence:

| Setting | Precedence |
| ------- | ---------- |
| Provider | Provider flags > `QSH_PROVIDER` > global config > `qsh.toml` > auto-detect |
| Backend | `QSH_BACKEND` > global config > `qsh.toml` > auto-detected CLI fallback > API default |
| Mode | CLI flag > `qsh.toml` > global config > env var > built-in default (`fast`) |
| Model | CLI `-m` > `qsh.toml` (`models.<mode>` then `model`) > global config (`models.<mode>` then `model`) > env var > built-in default |

`qsh.toml` is parsed as standard TOML.

### Global Config

Use the global config file when you want one place for provider preferences, API keys, model overrides, retry knobs, and timeouts.

```sh
# Initial setup. Pipe secrets in so they do not land in shell history.
qsh config set provider claude
qsh config set mode fast
echo "$ANTHROPIC_API_KEY" | qsh config set providers.claude.api_key
qsh config set providers.claude.model claude-sonnet-4-6
qsh config set providers.claude.backend cli

# Pick a different model per mode (overrides `providers.<p>.model`).
qsh config set providers.claude.models.fast  claude-haiku-4-5
qsh config set providers.claude.models.smart claude-opus-4-7

# Tune behavior.
qsh config set providers.openai.tokens.smart 32000
qsh config set providers.claude.tokens.thinking_budget 8000
qsh config set timeouts.smart_secs 240
qsh config set retry.keep 5

# Inspect effective config. API keys are redacted and sources are annotated.
qsh config show

# Open in $EDITOR.
qsh config edit
```

The first `qsh config set` or `qsh config edit` seeds a commented template at `${XDG_CONFIG_HOME:-~/.config}/qsh/config.toml` with mode `0600`. Later edits preserve comments.

Settable keys:

| Key | Type | Default |
| --- | ---- | ------- |
| `provider` | `gemini`, `openai`, `claude`, or `ollama` | auto-detect |
| `mode` | `fast` or `smart` | `fast` |
| `providers.<p>.api_key` | string, falls back to `<P>_API_KEY` env | unset |
| `providers.<openai|claude>.backend` | `api` or `cli` | `api` |
| `providers.<p>.model` | string, falls back to `<P>_MODEL` env and then built-in default | unset |
| `providers.<p>.models.fast` | string, per-mode model override for `fast` (wins over `model`) | unset |
| `providers.<p>.models.smart` | string, per-mode model override for `smart` (wins over `model`) | unset |
| `providers.<p>.tokens.fast` | u32 max output tokens | 1000 |
| `providers.<p>.tokens.smart` | u32 max output tokens | 16000, except Claude 10000 |
| `providers.claude.tokens.thinking_budget` | u32 Claude extended-thinking budget | 5000 |
| `providers.ollama.base_url` | string, falls back to `OLLAMA_BASE_URL` or `OLLAMA_HOST` | `http://127.0.0.1:11434` |
| `retry.keep` | failed attempts kept in replay history | 3 |
| `retry.window_minutes` | drop attempts older than this | 10 |
| `capture.stderr_bytes` | bytes of stderr stored per failure | 4096 |
| `timeouts.connect_secs` | connection timeout | 10 |
| `timeouts.fast_secs` | total request timeout in fast mode | 60 |
| `timeouts.smart_secs` | total request timeout in smart mode | 180 |

For API-key values, pipe the value via stdin. For non-secret keys, passing the value as the third argument is fine.

## Advanced Workflows

### Multiple Candidates

When the right answer is not obvious, ask for several candidate commands in a single request and pick one:

```sh
? --alts 4 dedupe lines from this file keeping the most recent
? -a 3 -c port-forward to staging
```

`qsh` asks the model to emit candidates separated by sentinel lines and parses those entries into a picker. It uses `fzf` when available and falls back to a numbered menu.

`--alts` results are not cached. The point is exploration, so identical future queries should still get fresh alternatives.

### Refine

Press `r` at the confirm prompt when a command is close but needs a targeted change. The model receives the original intent, the previous candidate, and the follow-up directive.

```text
$ ? find duplicate files
find . -type f -exec md5sum {} + | sort | uniq -d -w 32

Run?  [Y]es  [N]o  [E]dit  [R]efine  [?] r
refine: also exclude .git directory
find . -path ./.git -prune -o -type f -exec md5sum {} + | sort | uniq -d -w 32
```

Refines stack. Each `r` carries the prior candidate forward. Refines are not cached. Use `e` if you want your edited command to become the cached answer for the same query.

### Retry After Failure

When a command generated or edited through `?` fails, a bare `?` within 10 minutes replays the original intent plus recent attempts and stderr.

```text
$ ? extract this archive
tar -xzf archive.tar.bz2
Run?  [Y]es  [N]o  [E]dit  [R]efine  [?] y
gzip: stdin: not in gzip format
tar: Child returned status 1

$ ?
retrying: extract this archive
tar -xjf archive.tar.bz2
```

The retry buffer keeps up to 3 attempts. After a successful run it is cleared. After 10 minutes it expires.

### Edit Before Running

Press `e` to edit the candidate command before execution. The edited command is written to cache, so the same future query returns your edited version.

```text
$ ? show top memory hogs
ps aux --sort=-%mem | head -10
Run?  [Y]es  [N]o  [E]dit  [R]efine  [?] e
ps aux --sort=-%mem | head -20
```

### Explain Mode

`-e` or `--explain` appends a one-line `# why:` comment explaining the generated command.

```sh
? -e show all open ports with the owning process
? -e find files modified in the last week
? -e copy current branch name to clipboard
```

The wrapper strips the explanation before adding the command to shell history, so up-arrow gives the clean command.

## Reference

### Provider Selection

```sh
? -g find rust files                           # Gemini
? -c port-forward 8080 to staging              # Claude
? -o convert these png to webp                 # OpenAI
? -l -m qwen3:8b summarize disk usage          # Local Ollama
? -m claude-opus-4-1 -c plan a migration       # Override model
? -p openai -m gpt-5.4-pro design a pipeline   # Long-form provider flag
```

Ollama is selected when `OLLAMA_MODEL` is set or when `ollama list` returns an installed model. CLI fallback is selected only after API-key and Ollama checks.

### Flags

```text
Provider:
  -g, --gemini          Google Gemini      (env: GEMINI_API_KEY)
  -o, --openai          OpenAI             (env: OPENAI_API_KEY)
  -c, --claude          Anthropic Claude   (env: ANTHROPIC_API_KEY)
  -l, --local, --ollama Local Ollama       (env: OLLAMA_MODEL, no API key)
  -p, --provider PROV   One of gemini/openai/claude/ollama; aliases accepted

Backends:
  QSH_BACKEND=api|cli   Backend preference; CLI is supported for Claude/OpenAI
  qsh config set providers.claude.backend cli
  qsh config set providers.openai.backend cli

Mode:
  -s, --smart           Reasoning/thinking enabled, slower and more accurate
  -f, --fast            Minimal reasoning, fast and cheap

Cache:
  --no-cache            Skip the cache for this call
  QSH_NO_CACHE=1        Skip the cache for every call in this environment
  --clear-cache         Wipe ~/.cache/qsh and exit

Context:
  --no-context          Skip cwd-aware project-context injection
  QSH_NO_CONTEXT=1      Skip cwd-aware context for every call in this environment
  ./<path>              Include a file as labelled context, with a 32 KB cap

Alternatives:
  -a, --alts N          Ask for N candidates, 1-8, and pick one
  QSH_ALTS=N            Use N alternatives by default

Other:
  -m, --model MODEL     Override the model name for the chosen provider
  -e, --explain         Append a `# why:` comment explaining the command
  QSH_EXPLAIN=1         Append explanations by default
  -d, --debug           Print request body and raw response to stderr
  QSH_DEBUG=1           Enable debug dumps by default
  -h, --help            Show help
  --full-help           Print long-form help
```

### Environment Variables

| Variable | Meaning |
| -------- | ------- |
| `QSH_PROVIDER` | Default provider: `gemini`, `claude`, `openai`, or `ollama` |
| `QSH_BACKEND` | Backend preference: `api` or `cli`; CLI supports Claude/OpenAI |
| `QSH_MODE` | Default mode: `fast` or `smart` |
| `QSH_NO_CACHE` | Disable response-cache reads/writes when truthy |
| `QSH_NO_CONTEXT` | Disable cwd project-context injection when truthy |
| `QSH_EXPLAIN` | Append explanations by default when truthy |
| `QSH_DEBUG` | Enable debug dumps by default when truthy |
| `QSH_ALTS` | Default number of alternatives for `--alts` |
| `GEMINI_MODEL` | Override Gemini model, default `gemini-3.5-flash` |
| `OPENAI_MODEL` | Override OpenAI model, default `gpt-5.4-mini` |
| `ANTHROPIC_MODEL` | Override Claude model, default `claude-sonnet-4-6` |
| `OLLAMA_MODEL` | Override or select local Ollama model |
| `OLLAMA_HOST` | Override Ollama host |
| `OLLAMA_BASE_URL` | Override full Ollama/OpenAI-compatible base URL |
| `XDG_CACHE_HOME` | Cache root, default `~/.cache` |

### Requirements

- zsh 5.x+, bash 4+, or fish 3.x+ for the shell wrapper.
- An API key for at least one hosted provider, a local Ollama model, or a logged-in Claude Code/Codex CLI backend.

Optional tools used by `qsh` or by generated commands:

- `fzf` for alternative selection.
- `ollama` for local provider auto-detect.
- `claude` and `codex` for CLI backend auto-detect.
- `wl-copy`, `wl-paste`, `xclip`, `xsel`, `pbcopy`, or `pbpaste` for clipboard commands.
- `gh`, `ffmpeg`, `yt-dlp`, and other common command-line tools when relevant to the task.

## Safety And Privacy

The system prompt refuses common destructive patterns when the user has not explicitly named the target. Examples include recursive deletion of system roots, whole-disk writes to unnamed devices, fork bombs, pipe-to-shell from arbitrary URLs, broad `chmod 777`, and firewall-disabling commands.

If you explicitly name a specific path or device, such as `rm -rf /tmp/build` or a USB device path, `qsh` treats that as intentional and may generate the command.

This is defense in depth. You still see every command before it runs, and anything you confirm runs with your shell privileges.

Privacy notes:

- Prompts, stdin context, and file context are sent to the selected provider or backend.
- Local Ollama stays on the configured local endpoint.
- API keys are passed via HTTP headers, never URLs. Debug output redacts them.
- Stdin and file context are each capped at 32 KB.
- Debug mode prints the request body and raw response to stderr. It does not print provider headers, but the body includes your prompt and any context.

## How It Works

1. Parse flags, stdin, and task words.
2. Probe cwd for project context unless disabled.
3. Resolve provider, backend, model, mode, token budgets, retry settings, and timeouts.
4. Load stdin/file context within size caps.
5. Look up the response cache using provider, backend, model, mode, system prompt, and task context.
6. Stream a request to the selected provider/backend when there is no cache hit.
7. Strip markdown fences and other command wrappers.
8. Show the candidate on stderr and ask for confirmation.
9. Print only the accepted command to stdout.
10. Let the shell wrapper evaluate stdout in the current shell.
11. Capture stderr and record failed attempts for retry replay.

```text
user types:  ? list big files
    |
    v
qsh generate -- list big files
    stderr: spinner, status, command, confirm prompt
    stdout: accepted command only
    |
    v
shell wrapper:
    history-add the command
    eval it in the current shell
    capture stderr
    qsh record --status ...
```

### Cross-Platform Detection

The system prompt adjusts to the current environment. On every call, `qsh` probes:

- OS: `/etc/os-release` on Linux, `$OSTYPE` for macOS and BSDs.
- Package manager: `apt`, `dnf`, `pacman`, `apk`, `zypper`, `xbps-install`, `emerge`, `brew`, or `pkg`.
- Clipboard: Wayland, X11, macOS pasteboard, or none.
- Userland differences: BSD vs GNU flag conventions where relevant.

Detected families:

| Family | Package manager | Examples |
| ------ | --------------- | -------- |
| NixOS | declarative | NixOS |
| Debian | `apt` | Ubuntu, Debian, Mint, Pop!_OS, Kali, Raspbian |
| Fedora / RHEL | `dnf` | Fedora, RHEL, CentOS, Rocky, AlmaLinux, Amazon Linux |
| Arch | `pacman` | Arch, Manjaro, EndeavourOS, Garuda, Artix, CachyOS |
| Alpine | `apk` | Alpine |
| openSUSE | `zypper` | openSUSE, SLES |
| Void | `xbps-install` | Void |
| Gentoo | `emerge` | Gentoo |
| macOS | `brew` when installed | macOS |
| BSD | `pkg` | FreeBSD, OpenBSD, NetBSD, DragonFly |

Inspect what the model sees:

```sh
qsh generate -d -- ls files here 2>&1 | head -200
```

Debug output shows resolved provider/backend/model/mode, system-prompt size, cache file, and either the request body with redacted headers or the CLI command being run.

### Caching

Responses are cached in `${XDG_CACHE_HOME:-~/.cache}/qsh/`. The cache key includes provider, backend, model, mode, system prompt, and task context. Identical queries return instantly.

Edits made through `e` overwrite the cached entry, so your manual fix wins next time.

```sh
? --no-cache <query>
QSH_NO_CACHE=1 ? <query>
? --clear-cache
```

## Development

### Build And Test

```sh
cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo build --release
```

On NixOS where the C linker is not on `PATH`:

```sh
nix-shell -p gcc --run 'cargo build --release'
```

### Smoke-Test The Wrapper

```sh
target/release/qsh init zsh
zsh -n <(target/release/qsh init zsh)
eval "$(target/release/qsh init zsh)"
? find rust files modified today

target/release/qsh init bash
bash -n <(target/release/qsh init bash)

target/release/qsh init fish
target/release/qsh init fish | fish -n
```

### Inspect Requests

```sh
target/release/qsh generate -d -- ls files here 2>&1 | head -200
```

Debug output shows resolved provider/backend/model/mode, system-prompt size, cache file, and either the request body with redacted headers or the CLI command being run.

### Add A Provider

1. Add a variant to `Provider` in `src/config.rs`.
2. Add `src/providers/<name>.rs` with the provider request builder, env/model constants, and stream delta extraction.
3. Extend `api_key_env`, `default_model`, `model_env`, `stream_filter_kind`, `Provider::parse`, and the dispatch match in `src/providers/mod.rs`.
4. Add a `StreamKind` variant and cover it with an `extract_*_delta` unit test.

See `AGENTS.md` for the module layout and project invariants.
