//! Bottle = jedan Wine prefiks po igri, u `$BW_HOME/bottles/<id>`
//! (podrazumevano `~/Games/BridgeWine/bottles/<id>`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::doctor::find_wine;
use crate::recipe::{Graphics, Recipe, Store};

const STEAM_SETUP_URL: &str =
    "https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe";
const STEAM_EXE: &str = r"C:\Program Files (x86)\Steam\steam.exe";

pub fn home() -> PathBuf {
    if let Some(h) = std::env::var_os("BW_HOME") {
        return PathBuf::from(h);
    }
    let user_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    user_home.join("Games").join("BridgeWine")
}

pub fn bottle_path(home: &Path, id: &str) -> PathBuf {
    home.join("bottles").join(id)
}

/// Jedan poziv Wine-a: program, argumenti i env. Odvojeno od izvrsavanja radi testova i --dry-run.
#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

impl Invocation {
    fn command(&self) -> Command {
        let mut c = Command::new(&self.program);
        c.args(&self.args).envs(&self.env);
        c
    }

    pub fn display(&self) -> String {
        let env: Vec<String> = self
            .env
            .iter()
            .map(|(k, v)| format!("{k}={}", quote(v)))
            .collect();
        let mut parts = env;
        parts.push(quote(&self.program));
        parts.extend(self.args.iter().map(|a| quote(a)));
        parts.join(" ")
    }
}

fn quote(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./=,:".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

fn base_env(prefix: &Path) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert("WINEPREFIX".into(), prefix.display().to_string());
    env.insert("WINEDEBUG".into(), "-all".into());
    env
}

/// `wineboot --init` pravi prefiks; posle toga `winecfg -v` postavlja verziju Windowsa.
pub fn create_invocations(wine: &str, prefix: &Path, recipe: &Recipe) -> Vec<Invocation> {
    let env = base_env(prefix);
    vec![
        Invocation {
            program: wine.into(),
            args: vec!["wineboot".into(), "--init".into()],
            env: env.clone(),
        },
        Invocation {
            program: wine.into(),
            args: vec![
                "winecfg".into(),
                "-v".into(),
                recipe.runtime.windows_version.clone(),
            ],
            env,
        },
    ]
}

pub fn steam_install_invocation(wine: &str, prefix: &Path, setup: &Path) -> Invocation {
    let mut env = base_env(prefix);
    env.insert("WINEMSYNC".into(), "1".into());
    Invocation {
        program: wine.into(),
        args: vec![setup.display().to_string(), "/S".into()],
        env,
    }
}

pub fn run_invocation(wine: &str, prefix: &Path, recipe: &Recipe) -> Result<Invocation, String> {
    let mut env = base_env(prefix);
    env.extend(recipe.env.clone());
    if !recipe.dll_overrides.is_empty() {
        env.insert("WINEDLLOVERRIDES".into(), recipe.dll_overrides_string());
    }
    let args = match (
        recipe.game.store,
        recipe.game.steam_app_id,
        &recipe.game.exe,
    ) {
        (Store::Steam, Some(app), _) => {
            vec![STEAM_EXE.into(), "-applaunch".into(), app.to_string()]
        }
        (_, _, Some(exe)) => vec![exe.clone()],
        _ => return Err("recept nema ni steam_app_id ni exe".into()),
    };
    Ok(Invocation {
        program: wine.into(),
        args,
        env,
    })
}

fn run_all(invs: &[Invocation], dry_run: bool) -> Result<(), String> {
    for inv in invs {
        println!("$ {}", inv.display());
        if dry_run {
            continue;
        }
        let status = inv
            .command()
            .status()
            .map_err(|e| format!("ne mogu da pokrenem {}: {e}", inv.program))?;
        if !status.success() {
            return Err(format!("komanda nije uspela ({status})"));
        }
    }
    Ok(())
}

fn wine_or_err() -> Result<String, String> {
    find_wine().ok_or_else(|| "Wine (GPTK) nije pronadjen; pokreni `bw doctor`".into())
}

fn finish(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bw: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn cmd_create(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_or_err()?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if prefix.join("system.reg").exists() {
            println!("bottle vec postoji: {}", prefix.display());
            return Ok(());
        }
        if !dry_run {
            fs::create_dir_all(&prefix).map_err(|e| format!("{}: {e}", prefix.display()))?;
        }
        run_all(&create_invocations(&wine, &prefix, recipe), dry_run)?;
        println!("bottle spreman: {}", prefix.display());
        Ok(())
    })())
}

pub fn cmd_steam_install(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_or_err()?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if !dry_run && !prefix.join("system.reg").exists() {
            return Err(format!(
                "bottle ne postoji, prvo `bw bottle create {}`",
                recipe.game.id
            ));
        }
        let cache = home().join("cache");
        let setup = cache.join("SteamSetup.exe");
        if !setup.exists() {
            let curl: Vec<OsString> = vec![
                "-fL".into(),
                "-o".into(),
                setup.clone().into(),
                STEAM_SETUP_URL.into(),
            ];
            println!("$ curl -fL -o {} {STEAM_SETUP_URL}", setup.display());
            if !dry_run {
                fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
                let ok = Command::new("curl")
                    .args(&curl)
                    .status()
                    .map_err(|e| e.to_string())?
                    .success();
                if !ok {
                    return Err("preuzimanje SteamSetup.exe nije uspelo".into());
                }
            }
        }
        run_all(&[steam_install_invocation(&wine, &prefix, &setup)], dry_run)?;
        println!(
            "Steam instaliran. Sledece: `bw run {}`, uloguj se i instaliraj igru u Steamu.",
            recipe.game.id
        );
        Ok(())
    })())
}

pub fn cmd_run(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_or_err()?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if !dry_run && !prefix.join("system.reg").exists() {
            return Err(format!(
                "bottle ne postoji, prvo `bw bottle create {}`",
                recipe.game.id
            ));
        }
        if matches!(recipe.runtime.graphics, Graphics::Dxvk | Graphics::Dxmt) {
            eprintln!(
                "upozorenje: graphics = \"{}\" jos ne instaliramo automatski; koristi se ono sto je u bottle-u",
                recipe.runtime.graphics
            );
        }
        let inv = run_invocation(&wine, &prefix, recipe)?;
        run_all(&[inv], dry_run)
    })())
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: &str = r#"
[game]
id = "aoe3-de"
name = "AoE3 DE"
store = "steam"
steam_app_id = 933110

[runtime]
wine = "gptk"
graphics = "d3dmetal"
directx = 11

[env]
WINEMSYNC = "1"

[dll_overrides]
dxgi = "native,builtin"
"#;

    fn recipe() -> Recipe {
        Recipe::parse(R).unwrap()
    }

    #[test]
    fn bottle_lives_under_home() {
        assert_eq!(
            bottle_path(Path::new("/h"), "aoe3-de"),
            PathBuf::from("/h/bottles/aoe3-de")
        );
    }

    #[test]
    fn create_inits_prefix_then_sets_windows_version() {
        let inv = create_invocations("/w/wine64", Path::new("/p"), &recipe());
        assert_eq!(inv[0].args, ["wineboot", "--init"]);
        assert_eq!(inv[1].args, ["winecfg", "-v", "win10"]);
        assert_eq!(inv[0].env["WINEPREFIX"], "/p");
    }

    #[test]
    fn run_launches_through_steam_with_recipe_env() {
        let inv = run_invocation("/w/wine64", Path::new("/p"), &recipe()).unwrap();
        assert_eq!(inv.args, [STEAM_EXE, "-applaunch", "933110"]);
        assert_eq!(inv.env["WINEMSYNC"], "1");
        assert_eq!(inv.env["WINEDLLOVERRIDES"], "dxgi=native,builtin");
        assert_eq!(inv.env["WINEPREFIX"], "/p");
    }

    #[test]
    fn steam_install_is_silent() {
        let inv =
            steam_install_invocation("/w/wine64", Path::new("/p"), Path::new("/c/SteamSetup.exe"));
        assert_eq!(inv.args, ["/c/SteamSetup.exe", "/S"]);
    }

    #[test]
    fn display_quotes_spaces() {
        let inv = run_invocation(
            "/Applications/Game Porting Toolkit.app/wine64",
            Path::new("/p"),
            &recipe(),
        )
        .unwrap();
        let d = inv.display();
        assert!(
            d.contains("'/Applications/Game Porting Toolkit.app/wine64'"),
            "{d}"
        );
        assert!(
            d.contains(r"'C:\Program Files (x86)\Steam\steam.exe'"),
            "{d}"
        );
    }
}
