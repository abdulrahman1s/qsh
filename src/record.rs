use crate::cache;
use crate::cli::RecordArgs;
use crate::retry;
use std::path::Path;

pub fn run(args: RecordArgs) -> i32 {
    let dir = cache::cache_dir();
    let stderr = args.stderr_file.as_deref().map(Path::new);
    let original = if args.original_task.is_empty() {
        retry::load_last_task(&dir).unwrap_or_default()
    } else {
        args.original_task
    };
    if let Err(e) = retry::record(&dir, &args.cmd, stderr, args.status, &original) {
        crate::ui::warn(&format!("record failed: {e}"));
        return 1;
    }
    0
}
