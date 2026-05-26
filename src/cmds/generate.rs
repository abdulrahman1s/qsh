use super::cli::{GenerateArgs, Shell};
use crate::config::{ALTS_MAX, ALTS_MIN, Backend, INDICATOR_MAX, Mode, Provider, STDIN_CAP};
use crate::providers as provider;
use crate::providers::stream;
use crate::util::{
    alts, cache, clean, context, env_detect, files, project, prompt, retry,
    settings::{self, Settings},
    ui, xml_escape,
};
use std::io::{IsTerminal, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::watch;

pub async fn run(args: GenerateArgs) -> i32 {
    if args.full_help {
        print_help();
        return 0;
    }

    if args.clear_cache {
        cache::clear();
        println!("qsh: cache cleared");
        return 0;
    }

    let cache_dir = cache::cache_dir();

    // Persistent SIGINT handler — applies to every phase (streaming,
    // confirm prompt, edit). On the first Ctrl+C we hard-exit so the
    // user always sees a response.
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprint!("\r\x1b[K\x1b[2mqsh: cancelled\x1b[0m\n");
            let _ = std::io::Write::flush(&mut std::io::stderr());
            std::process::exit(130);
        }
    });

    let mut state: State = match build_state(args, &cache_dir) {
        Ok(s) => s,
        Err(rc) => return rc,
    };

    // Resolve cwd context once.
    if state.use_context {
        state.cwd_context = context::cwd_context();
    }

    let env = env_detect::detect();
    let sys_base = prompt::system_prompt(&env, state.shell);

    let mut next_action = NextAction::Generate;
    let mut cmd = String::new();

    while matches!(next_action, NextAction::Generate) {
        let task_full = if state.cwd_context.is_empty() {
            state.task.clone()
        } else {
            format!("<cwd>{}</cwd>\n\n{}", state.cwd_context, state.task)
        };

        // Build per-iteration system prompt.
        let directives = prompt::extra_directives(&prompt::DirectivesArgs {
            project_prompt: &state.project_prompt,
            retry: state.retry,
            refine: state.refine,
            explain: state.explain,
            alts: state.alts,
        });
        let sys = format!("{}{}", sys_base, directives);
        let max_tok = prompt::max_tokens(
            state.mode,
            state.explain,
            state.alts,
            state.provider,
            &state.settings,
        );

        let stop: Vec<String> = if state.alts > 1 {
            Vec::new()
        } else {
            vec!["\n\n".to_string()]
        };

        let req = provider::build(&provider::BuildArgs {
            provider: state.provider,
            backend: state.backend,
            system: &sys,
            task: &task_full,
            model: &state.model,
            mode: state.mode,
            max_tok,
            stop,
            settings: &state.settings,
        });

        // Cache key + lookup.
        let key = cache::key(
            state.provider.as_str(),
            state.backend.as_str(),
            &state.model,
            state.mode.as_str(),
            &sys,
            &task_full,
        );
        let cache_file = cache::file_for(&cache_dir, &key);
        let mut use_cache = state.use_cache;
        if state.alts > 1 {
            // Alts mode never caches.
            use_cache = false;
        }

        let mut cached_hit = false;
        if use_cache
            && cache_file.is_file()
            && let Some(c) = cache::load(&cache_file)
        {
            cmd = c;
            cached_hit = true;
        }

        if cached_hit {
            ui::status_line_cached(state.provider_label(), &state.model, state.mode.as_str());
            ui::print_command(&cmd);
            if state.debug {
                debug_dump(&state, &sys, &req, &cache_file, true);
            }
        } else {
            if state.alts > 1 && !state.retry && !state.refine {
                ui::status_line_alts(
                    state.provider_label(),
                    &state.model,
                    state.mode.as_str(),
                    state.alts,
                );
            } else {
                ui::status_line(state.provider_label(), &state.model, state.mode.as_str());
            }

            if state.debug {
                debug_dump(&state, &sys, &req, &cache_file, false);
            }

            // Cancellation infrastructure: kept for graceful drain on
            // network/HTTP errors. The top-level SIGINT handler exits
            // the process; cancellation through these channels is only
            // hit on internal short-circuit paths.
            let (_cancel_tx, cancel_rx) = watch::channel(false);
            let cancelled = Arc::new(AtomicBool::new(false));

            let mut handle = stream::start(req, state.mode, cancel_rx.clone(), &state.settings);
            let buf = Arc::clone(&handle.buf);

            ui::spinner_wait(
                Arc::clone(&buf),
                &mut handle.join,
                state.alts,
                state.retry,
                state.refine,
                Arc::clone(&cancelled),
            )
            .await;

            // For single-answer mode, do typewriter playback after spinner returns.
            let alts_mode = state.alts > 1 && !state.retry && !state.refine;
            if !alts_mode {
                ui::typewriter(
                    Arc::clone(&buf),
                    &mut handle.join,
                    Arc::clone(&cancelled),
                    cancel_rx.clone(),
                )
                .await;
            }

            let result = handle.join.await.unwrap_or(stream::StreamResult {
                text: String::new(),
                raw: String::new(),
                net_err: Some("task panicked".into()),
                status: None,
            });

            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return 130;
            }

            if state.debug {
                eprintln!("── qsh: raw response ──");
                eprintln!("{}", result.raw);
                eprintln!("── qsh: parsed text ──");
                eprintln!("{}", result.text);
                if let Some(e) = result.net_err.as_deref() {
                    eprintln!("── qsh: transport stderr ──\n{}", e);
                }
            }

            let cmd_buf = result.text.clone();
            if cmd_buf.is_empty() {
                let (kind, msg) = stream::classify_failure(&result.raw, result.net_err.as_deref());
                match kind {
                    stream::FailureKind::Network => ui::net_die(&msg),
                    stream::FailureKind::Api => ui::api_die(&msg),
                    stream::FailureKind::Parse => ui::parse_die(&msg),
                }
                return 1;
            }

            if alts_mode {
                let parsed = alts::parse(&cmd_buf, state.alts);
                if parsed.candidates.is_empty() {
                    ui::parse_die(
                        "model did not produce the expected sentinel format (try --debug)",
                    );
                    return 1;
                }
                if parsed.shortfall > 0 || parsed.dedupe_loss > 0 {
                    let mut why = Vec::new();
                    if parsed.shortfall > 0 {
                        why.push(format!("{} missing", parsed.shortfall));
                    }
                    if parsed.dedupe_loss > 0 {
                        why.push(format!("{} duplicate", parsed.dedupe_loss));
                    }
                    ui::info(&format!(
                        "{}/{} candidates ({})",
                        parsed.candidates.len(),
                        parsed.requested,
                        why.join(", ")
                    ));
                }
                let pick = ui::alts_picker(&parsed.candidates);
                match pick {
                    Some(c) if !c.is_empty() => {
                        cmd = c;
                        ui::print_command(&cmd);
                    }
                    _ => {
                        eprintln!("\r\x1b[K\x1b[2mqsh: no selection — aborted\x1b[0m");
                        return 0;
                    }
                }
            } else {
                cmd = clean::clean_command(&cmd_buf);
                if cmd.is_empty() {
                    ui::die("no command returned");
                    return 1;
                }
                if use_cache && state.alts == 1 {
                    let _ = cache::save(&cache_dir, &cache_file, &cmd);
                }
            }
        }

        // Confirm prompt.
        next_action = confirm(&mut cmd, &mut state, &cache_dir, &cache_file);
    }

    if let NextAction::Abort = next_action {
        return 0;
    }

    // Save original task next to .last_task so the wrapper can pass it
    // back to `record` even if the user editted the command.
    let _ = std::fs::create_dir_all(&cache_dir);
    let _ = std::fs::write(retry::last_task_file(&cache_dir), &state.original_task);

    // Final output: print the command to stdout for the shell wrapper to eval.
    let hist_cmd = clean::strip_why_comment(&cmd);
    println!("{}", hist_cmd_or_full(&cmd, &hist_cmd));
    0
}

fn hist_cmd_or_full(full: &str, _hist: &str) -> String {
    // The wrapper does history insertion itself with strip-why; pass the
    // FULL command (with comment) so user sees the explanation if -e
    // was used.
    full.to_string()
}

#[derive(Debug)]
enum NextAction {
    None,
    Generate,
    Abort,
}

struct State {
    provider: Provider,
    backend: Backend,
    model: String,
    mode: Mode,
    use_cache: bool,
    use_context: bool,
    debug: bool,
    explain: bool,
    alts: u32,
    shell: Shell,
    task: String,
    original_task: String,
    project_prompt: String,
    cwd_context: String,
    retry: bool,
    refine: bool,
    settings: Settings,
}

impl State {
    fn provider_label(&self) -> &'static str {
        match (self.provider, self.backend) {
            (Provider::Gemini, _) => "Gemini",
            (Provider::Openai, Backend::Api) => "OpenAI",
            (Provider::Openai, Backend::Cli) => "Codex",
            (Provider::Claude, _) => "Claude",
            (Provider::Ollama, _) => "Ollama",
        }
    }
}

fn task_and_file_refs(args: &[String]) -> (String, Vec<files::FileRef>) {
    let mut task_words = Vec::with_capacity(args.len());
    let mut file_refs = Vec::new();

    for arg in args {
        if let Some(fr) = files::parse_path_arg(arg) {
            task_words.push(fr.display.clone());
            file_refs.push(fr);
        } else {
            task_words.push(arg.clone());
        }
    }

    let user_task = task_words.join(" ").trim().to_string();
    (user_task, file_refs)
}

fn build_state(args: GenerateArgs, cache_dir: &Path) -> Result<State, i32> {
    if let Some(n) = args.alts
        && !(ALTS_MIN..=ALTS_MAX).contains(&n)
    {
        ui::die(&format!(
            "--alts needs an integer {}-{} (got: {})",
            ALTS_MIN, ALTS_MAX, n
        ));
        return Err(1);
    }

    // Load global config early — needed for retry window + everything below.
    let settings = settings::load();

    // Provider from explicit flag.
    let provider = if args.gemini {
        Some(Provider::Gemini)
    } else if args.openai {
        Some(Provider::Openai)
    } else if args.claude {
        Some(Provider::Claude)
    } else if args.ollama {
        Some(Provider::Ollama)
    } else if let Some(p) = args.provider.as_deref() {
        match Provider::parse(p) {
            Some(x) => Some(x),
            None => {
                ui::die(&format!(
                    "unknown provider: '{}' (use gemini, openai, claude, or ollama)",
                    p
                ));
                return Err(1);
            }
        }
    } else {
        None
    };

    // Mode from explicit flag.
    let mut mode: Option<Mode> = if args.smart {
        Some(Mode::Smart)
    } else if args.fast {
        Some(Mode::Fast)
    } else {
        None
    };

    let mut model: Option<String> = args.model.clone();

    // Collect resolved ./file refs for context and expand only those refs in
    // the user intent so commands can target them with absolute paths.
    let (user_task, file_refs) = task_and_file_refs(&args.task);

    // Stdin (if piped).
    let mut stdin_data = String::new();
    if !std::io::stdin().is_terminal() {
        let mut buf = Vec::with_capacity(STDIN_CAP);
        let mut h = std::io::stdin().take(STDIN_CAP as u64);
        if h.read_to_end(&mut buf).is_ok() {
            stdin_data = String::from_utf8_lossy(&buf).to_string();
        }
    }
    // Treat whitespace-only stdin as no stdin so the empty-input guard fires.
    if stdin_data.trim().is_empty() {
        stdin_data.clear();
    }

    // Read files for context.
    let file_block = if !file_refs.is_empty() {
        let b = files::read_files(&file_refs);
        for line in b.info.lines() {
            ui::info(line);
        }
        b
    } else {
        files::FileBlock {
            xml: String::new(),
            info: String::new(),
        }
    };
    let file_data = file_block.xml;

    // Retry-detect: no user input at all and recent attempts file.
    let mut retry = false;
    let mut use_cache = !args.no_cache;
    let task;
    let original_task;
    if user_task.is_empty()
        && stdin_data.is_empty()
        && file_data.is_empty()
        && retry::recent(cache_dir, &settings)
    {
        let attempts = retry::load_attempts(cache_dir);
        let last_task = retry::load_last_task(cache_dir).unwrap_or_default();
        let attempts_text = retry::format_attempts_for_prompt(&attempts);
        task = format!(
            "original intent:\n{}\n\nfailed prior attempts (oldest first):\n{}\n\nproduce a corrected single command.",
            if last_task.is_empty() {
                "(unknown)"
            } else {
                &last_task
            },
            attempts_text
        );
        original_task = last_task;
        use_cache = false;
        retry = true;

        let mut indicator = if original_task.is_empty() {
            "failed command".to_string()
        } else {
            original_task.clone()
        };
        if indicator.len() > INDICATOR_MAX {
            indicator = format!("{}...", &indicator[..INDICATOR_MAX.saturating_sub(3)]);
        }
        ui::retry_indicator(&indicator);
    } else {
        // Compose context envelope around user_task.
        let mut ctx = String::new();
        if !stdin_data.is_empty() {
            ctx.push_str(&format!(
                "<stdin context>\n{}\n</stdin context>\n",
                xml_escape::escape(&stdin_data)
            ));
        }
        if !file_data.is_empty() {
            ctx.push_str(&format!("<files context>\n{}</files context>\n", file_data));
        }
        if ctx.is_empty() {
            task = user_task.clone();
        } else {
            let hint = if !stdin_data.is_empty() && !file_refs.is_empty() {
                "(figure out what to do with the stdin and files)"
            } else if !stdin_data.is_empty() {
                "(figure out what to do with the stdin context)"
            } else {
                "(explain or operate on these files)"
            };
            let intent = if user_task.is_empty() {
                hint
            } else {
                &user_task
            };
            task = format!("{}\nuser intent: {}", ctx, intent);
        }
        let mut stub = String::new();
        if !stdin_data.is_empty() {
            stub.push_str("stdin ");
        }
        if let Some(f) = file_refs.first() {
            stub.push_str(&f.display);
            stub.push(' ');
        }
        original_task = if !user_task.is_empty() {
            user_task.clone()
        } else {
            stub.trim_end().to_string()
        };

        if task.is_empty() && stdin_data.is_empty() && file_data.is_empty() {
            print_help();
            return Err(1);
        }
    }

    // Load qsh.toml — provider/backend/model defaults are seeded below with
    // their own precedence chains.
    let mut project_prompt = String::new();
    let mut project_provider: Option<String> = None;
    let mut project_backend: Option<String> = None;
    let mut project_cfg: Option<project::ProjectConfig> = None;
    if let Some(path) = project::find() {
        let rc = project::load(&path);
        project_provider = rc.provider.clone();
        project_backend = rc.backend.clone();
        if mode.is_none() {
            mode = match rc.mode.as_deref() {
                Some("smart") => Some(Mode::Smart),
                Some("fast") => Some(Mode::Fast),
                _ => None,
            };
        }
        project_prompt = rc.prompt.clone();
        project_cfg = Some(rc);
    }

    // Resolve provider, backend, auth.
    let env_pref = std::env::var("QSH_PROVIDER").ok();
    let probe = provider::RealProbe;
    let resolved = provider::resolve_provider(
        provider,
        env_pref.as_deref(),
        project_provider.as_deref(),
        &mut model,
        &settings,
        &probe,
    );
    let Some((p, detected_backend)) = resolved else {
        ui::die(
            "no provider configured. Set one with `qsh config set provider <gemini|openai|claude|ollama>` then pipe an API key:\n  echo $KEY | qsh config set providers.<provider>.api_key\nOr export GEMINI_API_KEY / ANTHROPIC_API_KEY / OPENAI_API_KEY in your shell, use --ollama -m MODEL, or install/login to the claude or codex CLI.",
        );
        return Err(1);
    };

    let env_backend = std::env::var("QSH_BACKEND").ok();
    let backend = provider::resolve_backend(
        p,
        None,
        env_backend.as_deref(),
        &settings,
        project_backend.as_deref(),
        detected_backend,
    );

    if let Err(e) = provider::require_auth(p, backend, &settings, &probe) {
        ui::die(&e);
        return Err(1);
    }

    // Mode must be resolved before model so per-mode model overrides apply.
    let mode = mode
        .or(match settings.mode.as_deref() {
            Some("smart") => Some(Mode::Smart),
            Some("fast") => Some(Mode::Fast),
            _ => None,
        })
        .unwrap_or_else(|| match std::env::var("QSH_MODE").as_deref() {
            Ok("smart") => Mode::Smart,
            Ok("fast") => Mode::Fast,
            _ => Mode::Fast,
        });

    // Project config model seeds resolve_model only when CLI/auto-detect
    // didn't already produce a value.
    if model.is_none()
        && let Some(rc) = project_cfg.as_ref()
        && let Some(m) = rc.model_for(mode)
    {
        model = Some(m.to_string());
    }

    let model = match provider::resolve_model(p, model, mode, &settings) {
        Ok(m) => m,
        Err(e) => {
            ui::die(&e);
            return Err(1);
        }
    };

    let alts = args.alts.unwrap_or(1);

    Ok(State {
        provider: p,
        backend,
        model,
        mode,
        use_cache,
        use_context: !args.no_context,
        debug: args.debug,
        explain: args.explain,
        alts,
        shell: args.shell,
        task,
        original_task,
        project_prompt,
        cwd_context: String::new(),
        retry,
        refine: false,
        settings,
    })
}

fn confirm(cmd: &mut String, state: &mut State, cache_dir: &Path, cache_file: &Path) -> NextAction {
    loop {
        ui::confirm_prompt();
        let Some(key) = ui::read_tty_key() else {
            eprintln!();
            return NextAction::Abort;
        };
        // Raw mode disabled echo; mirror the keypress so the user sees what they pressed.
        match key {
            '\r' | '\n' => eprintln!(),
            c if (c as u32) >= 0x20 && (c as u32) < 0x7f => eprintln!("{}", c),
            _ => eprintln!(),
        }
        match key {
            'y' | 'Y' => return NextAction::None,
            'e' | 'E' => {
                let stripped = clean::strip_why_comment(cmd);
                match ui::vared_edit(&stripped) {
                    ui::EditResult::Accepted(edited) => {
                        *cmd = edited;
                        if state.use_cache && state.alts == 1 {
                            let _ = cache::save(cache_dir, cache_file, cmd);
                        }
                        return NextAction::None;
                    }
                    ui::EditResult::Cancelled => return NextAction::Abort,
                }
            }
            'r' | 'R' => {
                ui::refine_prompt();
                let Some(refinement) = ui::read_tty_line() else {
                    return NextAction::Abort;
                };
                if refinement.trim().is_empty() {
                    ui::warn("empty refinement, cancelling");
                    return NextAction::Abort;
                }
                let prev = clean::strip_why_comment(cmd);
                state.task = format!(
                    "original intent:\n{}\n\nprevious candidate:\n{}\n\nrefinement:\n{}\n\nproduce a corrected single command.",
                    if state.original_task.is_empty() {
                        "(unknown)"
                    } else {
                        &state.original_task
                    },
                    prev,
                    refinement
                );
                state.use_cache = false;
                state.refine = true;
                state.retry = false;
                return NextAction::Generate;
            }
            '?' | 'h' | 'H' => {
                ui::confirm_help();
                continue;
            }
            _ => return NextAction::Abort,
        }
    }
}

fn debug_dump(
    state: &State,
    sys: &str,
    inv: &provider::PreparedInvocation,
    cache_file: &Path,
    cached: bool,
) {
    eprintln!("── qsh: context ──");
    if let Ok(cwd) = std::env::current_dir() {
        eprintln!("cwd: {}", cwd.display());
    }
    eprintln!("provider: {}", state.provider.as_str());
    eprintln!("backend: {}", state.backend.as_str());
    eprintln!("model: {}", state.model);
    eprintln!("mode: {}", state.mode.as_str());
    eprintln!("shell: {}", state.shell.as_str());
    match inv {
        provider::PreparedInvocation::Http(req) => eprintln!("url: {}", req.url),
        provider::PreparedInvocation::Cli(cli) => {
            eprintln!("cli provider: {}", cli.provider.as_str());
            eprintln!("cli: {} {}", cli.program, cli.args.join(" "));
        }
    }
    eprintln!(
        "flags: cache={} context={} explain={} alts={} retry={} refine={}",
        state.use_cache as u32,
        state.use_context as u32,
        state.explain as u32,
        state.alts,
        state.retry as u32,
        state.refine as u32
    );
    eprintln!("system prompt bytes: {}", sys.len());
    eprintln!("task bytes: {}", state.task.len());
    eprintln!("cache file: {}", cache_file.display());
    eprintln!("cached: {}", cached);
    match inv {
        provider::PreparedInvocation::Http(req) => {
            eprintln!("headers:");
            for (k, v) in &req.headers {
                let redacted = match k.to_ascii_lowercase().as_str() {
                    "authorization" => "Bearer <redacted>".to_string(),
                    "x-api-key" | "x-goog-api-key" => "<redacted>".to_string(),
                    _ => v.clone(),
                };
                eprintln!("  {}: {}", k, redacted);
            }
            eprintln!("── qsh: request body ──");
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&req.body).unwrap_or_default()
            );
        }
        provider::PreparedInvocation::Cli(_) => {}
    }
}

fn print_help() {
    eprintln!(
        "Usage: ? [OPTIONS] <description>

Provider flags (default: auto-detect from API keys; override with $QSH_PROVIDER):
  -g, --gemini          Google Gemini      (env: GEMINI_API_KEY,    model: gemini-3.5-flash)
  -o, --openai          OpenAI             (env: OPENAI_API_KEY,    model: gpt-5.4-mini)
  -c, --claude          Anthropic Claude   (env: ANTHROPIC_API_KEY, model: claude-sonnet-4-6)
  -l, --ollama          Local Ollama       (env: OLLAMA_MODEL,      model: first installed)
  -p, --provider PROV   Same as the long form of the flags above

Backends:
  default               API backend; auto-detect prefers API keys before CLI tools
  QSH_BACKEND=cli       Prefer CLI backend for Claude/OpenAI when supported
  qsh.toml              backend = \"cli\"
  qsh config set providers.claude.backend cli
                        Use Claude Code CLI instead of the Anthropic API
  qsh config set providers.openai.backend cli
                        Use Codex CLI instead of the OpenAI API

Mode flags (default: fast; override with $QSH_MODE):
  -s, --smart           High reasoning/thinking — slower, more accurate
  -f, --fast            Minimal reasoning — fast and cheap (default)

Cache:
  --no-cache            Skip the cache for this call (no read, no write)
  QSH_NO_CACHE=1        Skip the cache for every call in this environment
  --clear-cache         Wipe ~/.cache/qsh and exit

Context:
  --no-context          Skip cwd-aware context (git branch, language manifests, build/test tooling)
  QSH_NO_CONTEXT=1      Skip cwd-aware context for every call in this environment
  ./<path>              Include the file at <path> as labelled context (first 32KB per file)
                          Slice: ./path:N first N lines, ./path:-N last N, ./path:A-B inclusive range
  qsh.toml              Per-project defaults (TOML) — searched up from cwd.

Alternatives:
  -a, --alts N          Ask the model for N (1-8) distinct candidate commands, pick via fzf
  QSH_ALTS=N            Use N alternatives by default

Retry & refine:
  ?                     Bare `?` within 10 min of a failed command retries with original intent
  [y/n/e/r]             At confirm prompt: r refines, e edits, y runs, n declines

Stdin:
  Anything piped to `?` is included as context.

Other:
  --shell SHELL        Wrapper shell context: zsh, bash, or fish (default: zsh)
  -m, --model MODEL     Override the model name for the chosen provider
  -e, --explain         Append a `# why: …` shell comment explaining the command
  QSH_EXPLAIN=1         Append explanations by default
  -d, --debug           Print context, request JSON, and raw response to stderr
  QSH_DEBUG=1           Enable debug dumps by default
  -h, --help            Show this help

Auto-detect order: gemini > claude > openai > ollama > claude CLI > codex CLI.
"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn rel_tmp_file(name: &str, body: &str) -> (String, String) {
        let rel_dir = format!("target/qsh-generate-test-{}", std::process::id());
        let dir = Path::new(&rel_dir);
        let _ = fs::create_dir_all(dir);
        let rel = format!("{}/{}", rel_dir, name);
        let mut f = fs::File::create(&rel).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        let arg = format!("./{}", rel);
        let abs = fs::canonicalize(&rel)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        (arg, abs)
    }

    #[test]
    fn task_keeps_file_refs_as_absolute_paths() {
        let (arg, abs) = rel_tmp_file("intent.txt", "hello\n");
        let args = vec!["summarize".to_string(), arg];

        let (task, refs) = task_and_file_refs(&args);

        assert_eq!(task, format!("summarize {}", abs));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].display, abs);
    }

    #[test]
    fn task_keeps_missing_dot_slash_refs_as_typed_without_file_context() {
        let rel = format!(
            "target/qsh-generate-test-{}/missing.txt",
            std::process::id()
        );
        let _ = fs::remove_file(&rel);
        let arg = format!("./{}", rel);
        let args = vec!["touch".to_string(), arg.clone()];

        let (task, refs) = task_and_file_refs(&args);

        assert_eq!(task, format!("touch {}", arg));
        assert!(refs.is_empty());
    }
}
