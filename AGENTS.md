# AGENTS.md

`qsh` is a zsh-first AI shell-command generator with bash and Fish wrappers.
It is a standalone binary that prints shell init code, generates one
candidate command at a time, records failed eval attempts, and keeps all
interactive UI on stderr.

## What kind of project this is

- **Daily-driver shell tool.** zsh is the primary target; bash and Fish
  are also supported through generated wrappers.
- **Cross-platform command generation.** Keep Linux, FreeBSD, and macOS
  working. Guard Linux-only assumptions, preserve Homebrew/macOS
  clipboard support, and account for BSD-vs-GNU flag differences.
- **Single binary.** `qsh` has subcommands such as `generate`, `record`,
  `init`, `known`, and `config`. There is no library surface.
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
  config.rs           constants (token budgets, timeouts, caps) and shared enums
  cmds/               command-facing modules
    cli.rs            clap definitions (Cli/Command/*Args)
    config.rs         config subcommand implementation
    generate.rs       glue: parse -> context -> resolve -> loop -> print
    init.rs           init zsh/bash/fish wrapper scripts
    known.rs          known subcommand implementation
    record.rs         record subcommand (called by wrapper)
  providers/          provider abstraction, request builders, stream parsing
    mod.rs            shared provider dispatch + model/key resolution
    gemini.rs         Gemini request body + SSE delta extraction
    openai.rs         OpenAI request body + SSE delta extraction
    claude.rs         Claude request body + SSE delta extraction
    ollama.rs         Ollama URL/model helpers + stream delta extraction
    stream.rs         reqwest streaming + SSE line parsing + error classification
  util/               shared helpers
    env_detect.rs     /etc/os-release, clipboard, pkg-manager detection
    context.rs        cwd hints (git branch, lang manifests, build tools)
    project.rs        walk-up qsh.toml parsing
    prompt.rs         prompt file loading + placeholder substitution
    files.rs          ./path file context (XML-escaped, 32K budget)
    cache.rs          sha256 cache keys, atomic save
    retry.rs          .last_attempts.jsonl history, 10-min window
    alts.rs           sentinel-delimited multi-candidate parse + dedupe
    clean.rs          strip fences, leading $/%, why-comment
    settings.rs       global config loading + effective defaults
    known.rs          known-program cache and categorization
    ui.rs             spinner, typewriter, confirm prompt, fzf picker
    xml_escape.rs     escape `& < > "` for embedding in <file>/<stdin> tags
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
2. Add `src/providers/<name>.rs` with the provider's request builder,
   env/model constants, and stream delta extraction.
3. Extend `api_key_env`, `default_model`, `model_env`,
   `stream_filter_kind`, `Provider::parse`, and the dispatch match in
   `src/providers/mod.rs`.
4. Add a `StreamKind` variant and cover it with an `extract_*_delta`
   unit test.

**Inspect what the model will see**

```bash
target/release/qsh generate -d -- ls files here 2>&1 | head -200
```

Debug dump shows resolved provider/model/mode, system-prompt size,
cache file, and the request body with redacted headers.

**Git commits**

Use the existing conventional-style subject format:

```text
feat(scope): short imperative summary
fix: short imperative summary
docs(scope): short imperative summary
chore: short imperative summary
chore: release vX.Y.Z
```

Keep the subject concise, lowercase the type, include a scope when it
adds useful context, and match the release-commit format exactly.

## Don'ts

- Don't add a config file outside `qsh.toml` (per-project, TOML) and the global `~/.config/qsh/config.toml`.
- Don't link in a TUI framework. Stderr ANSI escapes are sufficient;
  the confirm prompt reads from `/dev/tty` directly.
- Don't let anything except the final accepted command reach stdout
  from `generate`.
