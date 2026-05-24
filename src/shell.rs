use crate::cli::{InitArgs, Shell};

pub fn init(args: InitArgs) -> i32 {
    match args.shell {
        Shell::Bash => {
            println!("{}", BASH_INIT);
            0
        }
        Shell::Zsh => {
            println!("{}", ZSH_INIT);
            0
        }
    }
}

// Zsh wrapper. The strategy:
//   - `qsh` calls `qsh generate ...` and captures the accepted command
//     from stdout. Everything else (status, streaming, errors) goes
//     through stderr unchanged.
//   - `print -s --` pushes the command into the user's shell history so
//     up-arrow recall works naturally.
//   - eval runs the command in the *current* zsh process. Stderr is teed
//     into a tempfile so retry-state recording sees what failed.
//   - On exit we call `qsh record` so the JSONL is updated.
// The `?` alias gets `noglob` so the zsh glob doesn't eat the literal.
const ZSH_INIT: &str = r#"# qsh zsh integration. Source this from your zshrc:
#   eval "$(qsh init zsh)"

qsh() {
  case "$1" in
    generate|record|init|-h|--help|-V|--version)
      command qsh "$@"
      return $?
      ;;
  esac

  setopt LOCAL_OPTIONS LOCAL_TRAPS NO_NOTIFY NO_MULTIOS PIPE_FAIL NO_XTRACE NO_VERBOSE TYPESET_SILENT
  TRAPCHLD() { :; }

  local cmd_file err_file rc=0
  cmd_file=$(mktemp) || return 1
  err_file=$(mktemp) || { rm -f -- "$cmd_file"; return 1; }
  trap 'rm -f -- "$cmd_file" "$err_file" 2>/dev/null' EXIT

  command qsh generate --shell zsh "$@" >"$cmd_file"
  rc=$?
  if (( rc != 0 )); then
    return $rc
  fi

  local cmd
  cmd=$(< "$cmd_file")
  [[ -z "$cmd" ]] && return 0

  local hist_cmd="${cmd% [#]*}"
  hist_cmd="${hist_cmd%"${hist_cmd##*[![:space:]]}"}"
  [[ -n "$hist_cmd" ]] && print -s -- "$hist_cmd"

  { eval "$cmd" 3>&1 1>&4 2>&3 | tee -- "$err_file" >&2
    rc=${pipestatus[1]}
  } 4>&1

  local last_task=""
  [[ -f "$cmd_file.task" ]] && last_task=$(< "$cmd_file.task")

  command qsh record --cmd "$cmd" --status "$rc" --stderr-file "$err_file" --original-task "$last_task" >/dev/null 2>&1 || true
  return $rc
}

alias "?"="noglob qsh"
alias "??"="noglob qsh --smart"
"#;

// Bash wrapper. Bash has no zsh-style `noglob` precommand, so the `?`
// aliases expand to a tiny pre-step that disables globbing before Bash
// expands the natural-language arguments. `qsh` restores the caller's
// globbing option immediately, before the generated command is evaluated.
const BASH_INIT: &str = r#"# qsh bash integration. Source this from your bashrc:
#   eval "$(qsh init bash)"

__qsh_pre_noglob() {
  case $- in
    *f*) __QSH_HAD_NOGLOB=1 ;;
    *) __QSH_HAD_NOGLOB=0 ;;
  esac
  set -f
}

__qsh_restore_glob() {
  if [[ ${__QSH_HAD_NOGLOB+x} ]]; then
    if [[ $__QSH_HAD_NOGLOB == 0 ]]; then
      set +f
    fi
    unset __QSH_HAD_NOGLOB
  fi
}

qsh() {
  __qsh_restore_glob

  case "$1" in
    generate|record|init|-h|--help|-V|--version)
      command qsh "$@"
      return $?
      ;;
  esac

  local cmd_file err_file rc=0
  cmd_file=$(mktemp) || return 1
  err_file=$(mktemp) || { rc=$?; rm -f -- "$cmd_file"; return "$rc"; }

  command qsh generate --shell bash "$@" >"$cmd_file"
  rc=$?
  if (( rc != 0 )); then
    rm -f -- "$cmd_file" "$err_file"
    return "$rc"
  fi

  local cmd
  cmd=$(<"$cmd_file")
  if [[ -z "$cmd" ]]; then
    rm -f -- "$cmd_file" "$err_file"
    return 0
  fi

  local hist_cmd="${cmd% [#]*}"
  hist_cmd="${hist_cmd%"${hist_cmd##*[![:space:]]}"}"
  [[ -n "$hist_cmd" ]] && history -s -- "$hist_cmd"

  { eval "$cmd" 3>&1 1>&4 2>&3 | tee -- "$err_file" >&2
    rc=${PIPESTATUS[0]}
  } 4>&1

  local last_task=""
  [[ -f "$cmd_file.task" ]] && last_task=$(<"$cmd_file.task")

  command qsh record --cmd "$cmd" --status "$rc" --stderr-file "$err_file" --original-task "$last_task" >/dev/null 2>&1 || true
  rm -f -- "$cmd_file" "$err_file"
  return "$rc"
}

alias "?"="__qsh_pre_noglob; qsh"
alias "??"="__qsh_pre_noglob; qsh --smart"
"#;

#[cfg(test)]
mod tests {
    use super::{BASH_INIT, ZSH_INIT};

    #[test]
    fn zsh_init_does_not_reference_bash_glob_restore_helper() {
        assert!(!ZSH_INIT.contains("__qsh_restore_glob"));
        assert!(ZSH_INIT.contains(r#"alias "?"="noglob qsh""#));
    }

    #[test]
    fn bash_init_keeps_glob_restore_helper() {
        assert!(BASH_INIT.contains("__qsh_restore_glob()"));
        assert!(BASH_INIT.contains("qsh() {\n  __qsh_restore_glob\n\n  case \"$1\" in"));
        assert!(BASH_INIT.contains(r#"alias "?"="__qsh_pre_noglob; qsh""#));
    }
}
