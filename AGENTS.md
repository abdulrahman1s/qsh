# AGENTS.md

`qsh` is a zsh-first AI shell-command generator with bash and Fish wrappers.
It is a standalone binary that prints shell init code, generates one
candidate command at a time, records failed eval attempts, and keeps all
interactive UI on stderr.

## What kind of project this is

- **Daily-driver shell tool.** zsh is the primary target; bash and Fish
  are also supported through generated wrappers.
- **Single binary.** `qsh` has three subcommands: `generate`, `record`,
  and `init`. There is no library surface.
- **Shell wrapper holds eval/history.** `qsh generate` writes the
  accepted command to stdout; the wrapper printed by `qsh init zsh`,
  `qsh init bash`, or `qsh init fish` evaluates it in the user's current
  shell, captures stderr, and feeds retry state back via `qsh record`.
- **Stderr is the UI surface.** Status, spinner, typewriter playback,
  confirm prompt, debug dumps, and errors go to stderr. Stdout is
  reserved for the final command.

## Layout

```text
Cargo.toml            crate metadata + dependencies
prompts/              system prompt + mode directive text included at compile time
src/
  main.rs             entrypoint, tokio runtime, subcommand dispatch
  cli.rs              clap definitions (Cli/Command/{Generate,Record,Init}Args)
  config.rs           constants (token budgets, timeouts, caps) and shared enums
  env_detect.rs       /etc/os-release, clipboard, pkg-manager detection
  context.rs          cwd hints (git branch, lang manifests, build tools)
  qshrc.rs            walk-up .qshrc parsing
  prompt.rs           prompt file loading + placeholder substitution
  provider.rs         Gemini/OpenAI/Claude/Ollama abstraction + body builders
  stream.rs           reqwest streaming + SSE line parsing + error classification
  files.rs            ./path file context (XML-escaped, 32K budget)
  cache.rs            sha256 cache keys, atomic save
  retry.rs            .last_attempts.jsonl history, 10-min window
  alts.rs             sentinel-delimited multi-candidate parse + dedupe
  clean.rs            strip fences, leading $/%, why-comment
  ui.rs               spinner, typewriter, confirm prompt, fzf picker
  shell.rs            init zsh/bash/fish wrapper scripts
  record.rs           record subcommand (called by wrapper)
  generate.rs         glue: parse -> context -> resolve -> loop -> print
  xml_escape.rs       escape `& < > "` for embedding in <file>/<stdin> tags
```

## Invariants

Things that must stay true. Flag any change that would break one.

- **`qsh generate` stdout is the final command, nothing else.** All UX
  goes to stderr. The wrapper blindly evals stdout, so extra text would
  be a shell syntax error.
- **API keys never reach the URL or stdout.** They go in
  `Authorization`, `x-api-key`, or `x-goog-api-key` headers; debug
  output redacts them.
- **Retry/refine state lives under `$XDG_CACHE_HOME/qsh`** or
  `~/.cache/qsh`. JSONL format, 3-entry window, 10-minute freshness
  gate.
- **No backwards-compat shims.** This is a personal tool; behavior
  drift gets fixed in code, not papered over with legacy aliases or
  duplicate config paths.

## Common workflows

**Build and test**

```bash
cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo build --release
```

NixOS note: `cargo` itself works, but the C linker is not on a clean
PATH. Wrap builds in `nix-shell -p gcc --run '...'` when the link step
fails with `linker 'cc' not found`.

**Smoke-test the wrapper**

```bash
target/release/qsh init zsh
zsh -n <(target/release/qsh init zsh)
eval "$(target/release/qsh init zsh)"
? find rust files modified today

target/release/qsh init bash
bash -n <(target/release/qsh init bash)

target/release/qsh init fish
target/release/qsh init fish | fish -n
```

**Add a provider**

1. Add a variant to `Provider` in `src/config.rs`.
2. Extend `api_key_env`, `default_model`, `model_env`,
   `stream_filter_kind`, `Provider::parse`, and the match in `build`
   in `src/provider.rs`.
3. Add a `StreamKind` variant and `extract_delta` branch.
4. Cover with a `extract_*_delta` unit test.

**Inspect what the model will see**

```bash
target/release/qsh generate -d -- ls files here 2>&1 | head -200
```

Debug dump shows resolved provider/model/mode, system-prompt size,
cache file, and the request body with redacted headers.

## Don'ts

- Don't add a config file outside `.qshrc`.
- Don't link in a TUI framework. Stderr ANSI escapes are sufficient;
  the confirm prompt reads from `/dev/tty` directly.
- Don't let anything except the final accepted command reach stdout
  from `generate`.
