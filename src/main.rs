mod app;
mod editor;
mod herdr;

use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match (args.get(1).map(String::as_str), args.get(2)) {
        (Some("launch"), None) => herdr::launch(),
        (Some("edit"), Some(target)) if args.len() == 3 => app::run(target),
        _ => {
            eprintln!("usage: imebox launch | imebox edit <target-pane-id>");
            std::process::exit(2);
        }
    }
}
