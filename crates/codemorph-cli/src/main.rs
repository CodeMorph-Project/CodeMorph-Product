use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(
    name = "codemorph",
    version,
    about = "Verified source-to-source translation platform"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Check,
    Compile {
        language: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        standard: Option<String>,
        #[arg(long)]
        sanitize: Option<String>,
    },
    DiffTest {
        #[arg(long)]
        cpp_source: String,
        #[arg(long)]
        rust_source: String,
        #[arg(long)]
        input: Option<String>,
        #[arg(long, default_value = "1")]
        cases: usize,
        #[arg(long, default_value = "1")]
        seed: u64,
    },
    Benchmark {
        #[arg(long)]
        cpp_source: String,
        #[arg(long)]
        rust_source: String,
        #[arg(long)]
        input: Option<String>,
        #[arg(long, default_value = "20")]
        iterations: usize,
    },
    Migrate {
        #[arg()]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Models {
        #[command(subcommand)]
        command: ModelCommands,
    },
}

#[derive(Subcommand)]
enum ModelCommands {
    List,
    Validate,
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Check => commands::check(),
        Commands::Compile {
            language,
            source,
            standard,
            sanitize,
        } => commands::compile(language, source, standard, sanitize),
        Commands::DiffTest {
            cpp_source,
            rust_source,
            input,
            cases,
            seed,
        } => commands::diff_test(cpp_source, rust_source, input, cases, seed),
        Commands::Benchmark {
            cpp_source,
            rust_source,
            input,
            iterations,
        } => commands::benchmark(cpp_source, rust_source, input, iterations),
        Commands::Migrate { input, output } => commands::migrate(input, output),
        Commands::Models { command } => match command {
            ModelCommands::List => commands::models_list(),
            ModelCommands::Validate => commands::models_validate(),
        },
    }
}
