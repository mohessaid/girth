use clap::Parser;

/// Just-in-time, project-scoped agent skills.
#[derive(Parser)]
#[command(name = "girth", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
