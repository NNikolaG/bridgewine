//! BridgeWine CLI (`bw`).

mod doctor;
mod recipe;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "bw",
    version,
    about = "BridgeWine: Windows igre na Apple Silicon Macu"
)]
struct Cli {
    /// Folder sa receptima (podrazumevano: $BW_RECIPES_DIR ili ./recipes)
    #[arg(long, global = true)]
    recipes: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Proveri da li Mac ima sve sto treba
    Doctor,
    /// Rad sa receptima igara
    Recipe {
        #[command(subcommand)]
        action: RecipeAction,
    },
    /// Napravi Wine prefiks za igru (faza 1, u izradi)
    Bottle { game: String },
    /// Pokreni igru po receptu (faza 1, u izradi)
    Run { game: String },
}

#[derive(Subcommand)]
enum RecipeAction {
    /// Izlistaj sve recepte
    List,
    /// Prikazi jedan recept
    Show { id: String },
    /// Validiraj sve recepte (koristi se i u CI)
    Check,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let dir = recipe::recipes_dir(cli.recipes);

    match cli.command {
        Command::Doctor => doctor::run(),
        Command::Recipe { action } => match action {
            RecipeAction::List => recipe::cmd_list(&dir),
            RecipeAction::Show { id } => recipe::cmd_show(&dir, &id),
            RecipeAction::Check => recipe::cmd_check(&dir),
        },
        Command::Bottle { game } | Command::Run { game } => {
            eprintln!("bw: '{game}': jos nije implementirano, sledeci korak u fazi 1");
            ExitCode::from(2)
        }
    }
}
