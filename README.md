# Qsh ⚡

_qsh = "question shell." `?` was already the universal symbol for "I don't know. Figure it out." Now it actually does._

Yet another natural-language shell tool. Here's the one thing that makes this one stick:
![qsh demo](demo.gif)


> 🔁 **A bare `?` within 10 minutes of a failed command replays the original intent plus the last 3 attempts and their stderr.** The model fixes what actually broke instead of re-cycling through approaches it already tried. None of the other AI-shell tools I tried did this. It's the difference between a screenshot demo and something you reach for daily.

---

## What's actually in here

- **Failure-aware retries.** Bare `?` after a failed run replays intent plus stderr so the next candidate fixes the real error.
- **Edit-as-answer caching.** Tweak a command at the prompt and that edit becomes the remembered answer next time.
- **Single-request alternatives.** `--alts N` returns N distinct candidates in one round-trip — no re-prompting.
- **Distro-aware generation.** Detects your userland (Linux, BSD, macOS) so flag dialects match what you actually have.
- **`-e/--explain`.** Adds a short `# why:` note when you want to learn, not just run.
- **Project-aware context.** Pulls signals from cwd, piped stdin, and explicit `./file` references.
- **Safety hard-stops.** Refuses `rm -rf /` but lets `rm -rf /tmp/build` through — naming the target is taking responsibility.
- **Bring your own brain.** Hosted providers, local Ollama, and Claude/Codex CLI backends.

## Examples

### Fix on the fly

A failed command leaves a trail. A bare `?` within 10 minutes uses it.

```sh
$ ? rename all .jpeg files in this folder to .jpg
rename 's/\.jpeg$/.jpg/' *.jpeg                       [y/n/e/r] y
zsh: command not found: rename

$ ?
for f in *.jpeg; do mv "$f" "${f%.jpeg}.jpg"; done    [y/n/e/r]
```

### Pipe in context

Anything on stdin becomes part of the prompt.

```sh
$ git status | ? what should I do next
git add -A && git commit -m "wip: parser refactor"    [y/n/e/r]

$ kubectl get pods | ? which ones are unhealthy
kubectl get pods --field-selector=status.phase!=Running    [y/n/e/r]
```

### Point at a file

A leading `./path` is read and sent as XML-tagged context (32K cap).

```sh
$ ? ./Cargo.toml bump tokio to the latest minor
sed -i 's/^tokio = ".*"/tokio = "1.42"/' Cargo.toml    [y/n/e/r]
```

### Ask for alternatives

One round-trip, N distinct candidates.

```sh
$ ? --alts 3 count unique IPs in this access log
awk '{print $1}' access.log | sort -u | wc -l                 [y/n/e/r]
cut -d' ' -f1 access.log | sort | uniq | wc -l                [y/n/e/r]
awk '!seen[$1]++{c++} END{print c}' access.log                [y/n/e/r]
```

### Learn while you run

`-e/--explain` appends a one-line `# why:` so you actually pick up the flag.

```sh
$ ? -e show all open ports with the owning process
ss -tulpn  # why: TCP+UDP listening sockets with PID/program    [y/n/e/r]
```

### Reach for `??` when it's harder

`?` is fast mode; `??` turns reasoning on for the multi-step stuff.

```sh
$ ?? find every file changed in the last 3 commits that still has a TODO
git diff --name-only HEAD~3 HEAD | xargs grep -l TODO    [y/n/e/r]
```

## Install

Install the latest release to `~/.local/bin/qsh`:

```sh
curl -fsSL https://raw.githubusercontent.com/abdulrahman1s/qsh/master/install.sh | sh
```

See [DOCS.md](DOCS.md#install) for installer flags (`--yes`, `--no-modify-rc`, per-shell overrides) and other install methods.

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

## At the prompt

Every generated command waits for one of:

| Key | Action |
| --- | ------ |
| `y` | Run the command |
| `n` | Decline |
| `e` | Edit before running |
| `r` | Refine with a follow-up instruction |
| `?` | Show prompt help |

Pressing Enter declines by default. Read the command before pressing `y`.

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
