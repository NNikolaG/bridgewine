//! BridgeWine CLI (`bw`).

mod bottle;
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
    /// Wine prefiks (bottle) za igru
    Bottle {
        #[command(subcommand)]
        action: BottleAction,
    },
    /// Windows Steam u bottle-u igre
    Steam {
        #[command(subcommand)]
        action: SteamAction,
    },
    /// Pokreni igru po receptu
    Run {
        game: String,
        /// Samo ispisi komandu, ne pokreci
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum BottleAction {
    /// Napravi prefiks i postavi verziju Windowsa iz recepta
    Create {
        game: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum SteamAction {
    /// Skini i tiho instaliraj Windows Steam
    Install {
        game: String,
        #[arg(long)]
        dry_run: bool,
    },
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
        Command::Bottle {
            action: BottleAction::Create { game, dry_run },
        } => with_recipe(&dir, &game, |r| bottle::cmd_create(r, dry_run)),
        Command::Steam {
            action: SteamAction::Install { game, dry_run },
        } => with_recipe(&dir, &game, |r| bottle::cmd_steam_install(r, dry_run)),
        Command::Run { game, dry_run } => with_recipe(&dir, &game, |r| bottle::cmd_run(r, dry_run)),
    }
}

fn with_recipe(
    dir: &std::path::Path,
    id: &str,
    f: impl FnOnce(&recipe::Recipe) -> ExitCode,
) -> ExitCode {
    match recipe::load_file(&dir.join(format!("{id}.toml"))) {
        Ok(r) => f(&r),
        Err(errs) => {
            eprintln!("bw: recept '{id}' nije ucitan:");
            for e in errs {
                eprintln!("  {e}");
            }
            ExitCode::FAILURE
        }
    }
}
