use super::cli::RecordArgs;
use crate::util::{cache, retry, settings, ui};
use std::path::Path;

pub fn run(args: RecordArgs) -> i32 {
    let dir = cache::cache_dir();
    let stderr = args.stderr_file.as_deref().map(Path::new);
    let original = if args.original_task.is_empty() {
        retry::load_last_task(&dir).unwrap_or_default()
    } else {
        args.original_task
    };
    let s = settings::load();
    if let Err(e) = retry::record(&dir, &args.cmd, stderr, args.status, &original, &s) {
        ui::warn(&format!("record failed: {e}"));
        return 1;
    }
    0
}
