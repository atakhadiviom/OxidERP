use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "oxiderp", version, about = "OxidERP developer and admin CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new module scaffold
    ModuleNew { name: String },
    /// Validate a module package or wasm file
    ModuleValidate { path: String },
    /// Print quick start instructions
    Doctor,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::ModuleNew { name } => println!("Create module scaffold: {name}"),
        Commands::ModuleValidate { path } => println!("Validate module artifact: {path}"),
        Commands::Doctor => println!("OxidERP CLI ready. Run: cargo run -p oxiderp-core"),
    }
}
