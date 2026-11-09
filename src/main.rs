#[path = "../crates/cli/cli.rs"]
mod cli;
#[path = "../crates/provider/OpenAIProvider.rs"]
mod provider;
#[path = "../crates/tools/tools.rs"]
mod tools;

fn main() {
    // Load .env (searching the current dir and its parents). Variables already
    // set in the shell take precedence; a missing .env file is not an error.
    let _ = dotenvy::dotenv();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(msg) = cli::run(&args) {
        eprintln!("{msg}");
        std::process::exit(1);
    }
}
