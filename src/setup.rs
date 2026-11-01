//! `bw setup <igra>`: sve sto recept trazi, jednom komandom i bez ponavljanja
//! onoga sto vec postoji. Posle toga ostaje samo `bw run <igra>`.
//!
//! Redosled: Mac provere -> Wine runtime (+ skidanje karantina) -> nasi Wine moduli
//! (release, pa build iz izvora ako release ne postoji) -> grafika (DXMT) -> bottle
//! -> Windows Steam. WPAD/DNS zaobilazenje, steamwebhelper wrapper, ciscenje GPTK
//! DLL-ova i kopiranje DXMT-a u bottle radi `bw run` pri svakom startu, jer ih
//! Steam i Wine mogu vratiti.

use std::path::Path;
use std::process::{Command, ExitCode};

use crate::bottle::{self, bottle_path, home, run_all, Invocation};
use crate::doctor::{self, Status};
use crate::dxmt;
use crate::recipe::{Recipe, Store, WineSource};
use crate::runtime;
use crate::wine_build;

/// Koraci koje `setup` radi za recept, redom (za ispis i testove).
pub fn plan(r: &Recipe) -> Vec<&'static str> {
    let mut v = vec!["Mac provere"];
    v.push(match r.runtime.wine {
        WineSource::Winehq => "WineHQ runtime",
        WineSource::Gptk => "Wine (GPTK)",
        WineSource::System => "Wine (BW_WINE)",
    });
    if r.runtime.patched {
        v.push("BridgeWine Wine moduli");
    }
    if r.runtime.graphics.needs_dxmt_runtime() {
        v.push("DXMT");
    }
    v.push("Bottle");
    if r.game.store == Store::Steam {
        v.push("Windows Steam");
    }
    v
}

/// Mac provere koje blokiraju svaku igru (ostale su informativne).
const BLOCKING: &[&str] = &["macOS", "Rosetta 2", "Homebrew prava"];

fn mac_checks() -> Result<(), String> {
    let bad: Vec<_> = doctor::evaluate(&doctor::probe())
        .into_iter()
        .filter(|c| c.status == Status::Fail && BLOCKING.contains(&c.name))
        .collect();
    if bad.is_empty() {
        return Ok(());
    }
    let mut msg = String::from("Mac nije spreman:");
    for c in bad {
        msg.push_str(&format!("\n  {}: {}", c.name, c.detail));
        if let Some(f) = c.fix {
            msg.push_str(&format!("\n    -> {f}"));
        }
    }
    Err(msg)
}

fn quarantined(app: &Path) -> bool {
    Command::new("xattr")
        .args(["-p", "com.apple.quarantine"])
        .arg(app.join("Contents/Resources/wine/bin/wine"))
        .output()
        .is_ok_and(|o| o.status.success())
}

pub fn unquarantine_invocation(app: &Path) -> Invocation {
    Invocation {
        program: "xattr".into(),
        args: vec![
            "-dr".into(),
            "com.apple.quarantine".into(),
            app.display().to_string(),
        ],
        env: Default::default(),
    }
}

fn patched_modules(app: &Path, dry_run: bool) -> Result<(), String> {
    if !dry_run && wine_build::is_installed(app) {
        println!("vec instalirani");
        return Ok(());
    }
    match wine_build::install_prebuilt(dry_run) {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("{e}\ngradim iz izvornog koda (traje, treba Xcode CLT + mingw)...");
            wine_build::build(dry_run, None)
        }
    }
}

pub fn setup(r: &Recipe, dry_run: bool) -> Result<(), String> {
    let steps = plan(r);
    let total = steps.len();
    let mut n = 0;
    let mut step = |name: &str| {
        n += 1;
        println!("\n== [{n}/{total}] {name} ==");
    };

    step(steps[0]);
    if dry_run {
        println!("(preskoceno uz --dry-run)");
    } else {
        mac_checks()?;
        println!("ok");
    }

    step(steps[1]);
    if r.runtime.wine == WineSource::Winehq {
        runtime::install(dry_run)?;
    }
    let wine = bottle::wine_for(r, dry_run)?;
    println!("wine: {wine}");
    let app = wine_build::app_for(&wine);
    if let Some(app) = &app {
        // nas runtime u $BW_HOME: karantin skidamo sami (stari bw ili rucno raspakovan tarball)
        if r.runtime.wine == WineSource::Winehq && !dry_run && quarantined(app) {
            run_all(&[unquarantine_invocation(app)], false)?;
        }
    }

    if r.runtime.patched {
        step("BridgeWine Wine moduli");
        let app = app.as_deref().ok_or("ne znam gde je Wine runtime")?;
        patched_modules(app, dry_run)?;
    }

    if r.runtime.graphics.needs_dxmt_runtime() {
        step("DXMT");
        let app = app.as_deref().ok_or("ne znam gde je Wine runtime")?;
        dxmt::ensure(app, dry_run)?;
        println!("DXMT {}", dxmt::DXMT_VERSION);
    }

    step("Bottle");
    bottle::create(r, dry_run)?;

    let prefix = bottle_path(&home(), &r.game.id);
    if r.game.store == Store::Steam {
        step("Windows Steam");
        if bottle::steam_installed(&prefix) {
            println!("vec instaliran");
        } else {
            bottle::steam_install(r, dry_run)?;
        }
    }

    println!("\nGotovo. Pokreni: bw run {}", r.game.id);
    if let (Store::Steam, Some(app_id)) = (r.game.store, r.game.steam_app_id) {
        if !bottle::steam_game_installed(&prefix, app_id) {
            println!(
                "Prvi put: uloguj se u Steam; on ce ponuditi instalaciju igre ({app_id}). Posle instalacije opet `bw run {}`.",
                r.game.id
            );
        }
    }
    println!("Provera stanja: bw doctor {}", r.game.id);
    Ok(())
}

pub fn cmd_setup(r: &Recipe, dry_run: bool) -> ExitCode {
    match setup(r, dry_run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bw: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aoe3() -> Recipe {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("recipes/aoe3-de.toml");
        crate::recipe::load_file(&p).unwrap()
    }

    #[test]
    fn aoe3_plan_covers_everything() {
        assert_eq!(
            plan(&aoe3()),
            [
                "Mac provere",
                "WineHQ runtime",
                "BridgeWine Wine moduli",
                "DXMT",
                "Bottle",
                "Windows Steam"
            ]
        );
    }

    #[test]
    fn unquarantine_is_recursive_on_the_app() {
        let inv = unquarantine_invocation(Path::new("/h/runtimes/wine-devel-11.21.app"));
        assert_eq!(inv.program, "xattr");
        assert_eq!(
            inv.args,
            [
                "-dr",
                "com.apple.quarantine",
                "/h/runtimes/wine-devel-11.21.app"
            ]
        );
    }
}
