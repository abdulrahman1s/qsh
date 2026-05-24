use super::cli::KnownArgs;
use crate::util::known;

pub fn run(args: KnownArgs) -> i32 {
    let list = if args.refresh {
        known::refresh()
    } else {
        known::load_or_refresh()
    };
    eprintln!(
        "qsh: {} ({} programs)",
        known::known_path().display(),
        list.len()
    );
    for name in &list {
        println!("{name}");
    }
    0
}
