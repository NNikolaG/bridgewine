//! `bw doctor`: proverava da li Mac ima sve sto treba za BridgeWine.

use std::path::Path;
use std::process::{Command, ExitCode};

use crate::bottle;
use crate::dxmt;
use crate::recipe::{CefWorkaround, Graphics, Recipe, Store, WineSource};
use crate::runtime;
use crate::wine_build;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    pub fix: Option<String>,
}

/// Rezultati sondiranja sistema; odvojeni od logike da bi se testirali na Linuxu.
#[derive(Debug, Default)]
pub struct Probe {
    pub os: String,
    pub arch: String,
    pub macos_version: Option<String>,
    pub rosetta: bool,
    pub xcode_clt: bool,
    pub brew_arm: bool,
    /// /opt/homebrew postoji ali trenutni korisnik ne sme da pise u njega.
    pub brew_not_writable: bool,
    pub brew_x86: bool,
    pub gptk_wine: Option<String>,
    /// WineHQ build iz `$BW_HOME/runtimes` (za 64-bitni Steam).
    pub winehq_wine: Option<String>,
    pub d3dmetal: Option<String>,
}

pub const GPTK_WINE_PATHS: &[&str] = &[
    "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64",
    "/usr/local/opt/game-porting-toolkit/bin/wine64",
    "/usr/local/bin/wine64",
    "/opt/homebrew/bin/wine64",
];

const D3DMETAL_PATHS: &[&str] = &[
    "/usr/local/opt/game-porting-toolkit/lib/external/D3DMetal.framework",
    "/Volumes/Evaluation environment for Windows games 2.1/redist/lib/external/D3DMetal.framework",
    "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/lib/external/D3DMetal.framework",
];

fn cmd_ok(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn cmd_out(prog: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(prog).args(args).output().ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn first_existing(paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| p.to_string())
}

/// Wine koji BridgeWine koristi: $BW_WINE ako postoji, inace prvi GPTK wine64.
pub fn find_wine() -> Option<String> {
    find_wine_for(WineSource::Gptk)
}

fn bw_wine() -> Option<String> {
    std::env::var("BW_WINE")
        .ok()
        .filter(|p| Path::new(p).exists())
}

/// Wine za recept: $BW_WINE uvek ima prednost, inace po izvoru iz recepta.
pub fn find_wine_for(src: WineSource) -> Option<String> {
    bw_wine().or_else(|| match src {
        WineSource::Gptk => first_existing(GPTK_WINE_PATHS),
        WineSource::Winehq => runtime::winehq_wine(),
        WineSource::System => None,
    })
}

pub fn probe() -> Probe {
    let os = std::env::consts::OS.to_string();
    let mac = os == "macos";
    Probe {
        arch: cmd_out("uname", &["-m"]).unwrap_or_else(|| std::env::consts::ARCH.into()),
        macos_version: if mac {
            cmd_out("sw_vers", &["-productVersion"])
        } else {
            None
        },
        rosetta: mac && cmd_ok("arch", &["-x86_64", "/usr/bin/true"]),
        xcode_clt: mac && cmd_ok("xcode-select", &["-p"]),
        brew_arm: Path::new("/opt/homebrew/bin/brew").exists(),
        brew_not_writable: Path::new("/opt/homebrew").exists()
            && !cmd_ok("test", &["-w", "/opt/homebrew"]),
        brew_x86: mac && Path::new("/usr/local/bin/brew").exists(),
        gptk_wine: find_wine(),
        winehq_wine: runtime::winehq_wine(),
        d3dmetal: first_existing(D3DMETAL_PATHS),
        os,
    }
}

fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

pub fn evaluate(p: &Probe) -> Vec<Check> {
    use Status::*;
    let mut v = Vec::new();

    if p.os != "macos" {
        v.push(Check {
            name: "macOS",
            status: Fail,
            detail: format!("ovo je {}, BridgeWine radi samo na macOS-u", p.os),
            fix: None,
        });
        return v;
    }
    let ver = p
        .macos_version
        .clone()
        .unwrap_or_else(|| "nepoznato".into());
    let (st, fix) = match p.macos_version.as_deref().and_then(major) {
        Some(m) if m >= 14 => (Ok, None),
        Some(_) => (
            Fail,
            Some(String::from(
                "GPTK i D3DMetal traze macOS 14 Sonoma ili noviji",
            )),
        ),
        None => (
            Warn,
            Some(String::from("ne mogu da procitam verziju (sw_vers)")),
        ),
    };
    v.push(Check {
        name: "macOS",
        status: st,
        detail: ver,
        fix,
    });

    let apple = p.arch == "arm64";
    v.push(Check {
        name: "Apple Silicon",
        status: if apple { Ok } else { Warn },
        detail: p.arch.clone(),
        fix: (!apple)
            .then(|| "BridgeWine cilja M-serija cipove; na Intel Macu D3DMetal ne radi".into()),
    });
    if apple {
        v.push(Check {
            name: "Rosetta 2",
            status: if p.rosetta { Ok } else { Fail },
            detail: if p.rosetta {
                "instalirana".into()
            } else {
                "nije instalirana".into()
            },
            fix: (!p.rosetta).then(|| "softwareupdate --install-rosetta --agree-to-license".into()),
        });
    }
    v.push(Check {
        name: "Xcode CLT",
        status: if p.xcode_clt { Ok } else { Fail },
        detail: if p.xcode_clt {
            "instaliran".into()
        } else {
            "nije instaliran".into()
        },
        fix: (!p.xcode_clt).then(|| "xcode-select --install".into()),
    });
    v.push(Check {
        name: "Homebrew",
        status: if p.brew_arm || p.brew_x86 { Ok } else { Warn },
        detail: match (p.brew_arm, p.brew_x86) {
            (true, true) => "arm64 (/opt/homebrew) i x86_64 (/usr/local)".into(),
            (true, false) => "arm64 (/opt/homebrew)".into(),
            (false, true) => "x86_64 (/usr/local)".into(),
            (false, false) => "nije pronadjen".into(),
        },
        fix: (!p.brew_arm && !p.brew_x86).then(|| "https://brew.sh".into()),
    });
    if p.brew_not_writable {
        v.push(Check {
            name: "Homebrew prava",
            status: Fail,
            detail: "/opt/homebrew nije upisiv za tvog korisnika".into(),
            fix: Some(String::from(
                "sudo chown -R $(whoami) /opt/homebrew  (nikad `sudo brew`)",
            )),
        });
    }

    v.push(Check {
        name: "Wine (GPTK)",
        status: if p.gptk_wine.is_some() { Ok } else { Fail },
        detail: p.gptk_wine.clone().unwrap_or_else(|| "nije pronadjen".into()),
        fix: p.gptk_wine.is_none().then(|| {
            "brew install --cask gcenx/wine/game-porting-toolkit, pa xattr -dr com.apple.quarantine \"/Applications/Game Porting Toolkit.app\" (docs/install-gptk.md); ili postavi BW_WINE".into()
        }),
    });
    v.push(Check {
        name: "Wine (WineHQ)",
        status: if p.winehq_wine.is_some() { Ok } else { Warn },
        detail: p
            .winehq_wine
            .clone()
            .unwrap_or_else(|| "nije instaliran (treba za recepte sa wine = \"winehq\")".into()),
        fix: p.winehq_wine.is_none().then(|| "bw runtime install".into()),
    });
    v.push(Check {
        name: "D3DMetal",
        status: if p.d3dmetal.is_some() { Ok } else { Warn },
        detail: p
            .d3dmetal
            .clone()
            .unwrap_or_else(|| "nije pronadjen (DXVK/WineD3D i dalje rade)".into()),
        fix: p
            .d3dmetal
            .is_none()
            .then(|| "potreban samo za DX12 i recepte sa graphics = \"d3dmetal\"".into()),
    });
    v
}

/// Stanje jedne igre (runtime, bottle, Steam) za `bw doctor <igra>`.
#[derive(Debug, Default)]
pub struct GameProbe {
    pub wine: Option<String>,
    /// `com.apple.quarantine` na runtime-u (Gatekeeper ubija wine).
    pub quarantined: bool,
    pub patched_modules: bool,
    pub dxmt_runtime: bool,
    pub bottle: Option<String>,
    pub reg_keys: usize,
    pub dxmt_bottle: bool,
    pub stale_dlls: Vec<String>,
    pub steam: bool,
    pub helper_wrapped: bool,
    pub game_installed: bool,
}

fn quarantined(path: &Path) -> bool {
    cmd_ok(
        "xattr",
        &["-p", "com.apple.quarantine", &path.display().to_string()],
    )
}

pub fn probe_game(r: &Recipe) -> GameProbe {
    let wine = find_wine_for(r.runtime.wine);
    let app = wine.as_deref().and_then(wine_build::app_for);
    let prefix = bottle::bottle_path(&bottle::home(), &r.game.id);
    let has_bottle = bottle::bottle_exists(&prefix);
    GameProbe {
        quarantined: std::env::consts::OS == "macos"
            && wine.as_deref().is_some_and(|w| quarantined(Path::new(w))),
        patched_modules: app.as_deref().is_some_and(wine_build::is_installed),
        dxmt_runtime: app
            .as_deref()
            .is_some_and(|a| dxmt::is_installed(a, dxmt::DXMT_VERSION)),
        dxmt_bottle: has_bottle
            && app
                .as_deref()
                .is_some_and(|a| dxmt::bottle_synced(a, &prefix)),
        bottle: has_bottle.then(|| prefix.display().to_string()),
        reg_keys: bottle::registry_keys(&prefix),
        stale_dlls: bottle::stale_builtins(&prefix)
            .into_iter()
            .filter_map(|(_, p)| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect(),
        steam: bottle::steam_installed(&prefix),
        helper_wrapped: bottle::steam_helper_wrapped(&prefix),
        game_installed: r
            .game
            .steam_app_id
            .is_some_and(|id| bottle::steam_game_installed(&prefix, id)),
        wine,
    }
}

fn check(name: &'static str, ok: bool, bad: Status, detail: String, fix: &str) -> Check {
    Check {
        name,
        status: if ok { Status::Ok } else { bad },
        detail,
        fix: (!ok).then(|| fix.to_string()),
    }
}

fn yes_no(ok: bool, yes: &str, no: &str) -> String {
    if ok { yes } else { no }.to_string()
}

/// Sta igri fali. Sve sto je `FALI` resava `bw setup <igra>`; `upoz` popravi i `bw run`.
pub fn evaluate_game(r: &Recipe, p: &GameProbe) -> Vec<Check> {
    use Status::*;
    let id = &r.game.id;
    let setup = format!("bw setup {id}");
    let run = format!("bw run {id} (radi to sam pri svakom startu)");
    let mut v = Vec::new();
    let wine_fix = match r.runtime.wine {
        WineSource::Winehq => setup.clone(),
        WineSource::Gptk => "bw doctor (Wine (GPTK))".into(),
        WineSource::System => "postavi BW_WINE=<putanja do wine>".into(),
    };
    v.push(check(
        "Wine",
        p.wine.is_some(),
        Fail,
        p.wine.clone().unwrap_or_else(|| "nije pronadjen".into()),
        &wine_fix,
    ));
    if p.wine.is_none() {
        return v;
    }
    if p.quarantined {
        v.push(Check {
            name: "Karantin",
            status: Fail,
            detail: "runtime ima com.apple.quarantine (Gatekeeper ubija wine)".into(),
            fix: Some(format!(
                "xattr -dr com.apple.quarantine \"{}\"",
                p.wine
                    .as_deref()
                    .and_then(wine_build::app_for)
                    .map(|a| a.display().to_string())
                    .unwrap_or_default()
            )),
        });
    }
    if r.runtime.patched {
        v.push(check(
            "Wine moduli",
            p.patched_modules,
            Fail,
            yes_no(
                p.patched_modules,
                "BridgeWine patch-evi",
                "nema nasih patch-eva",
            ),
            &setup,
        ));
    }
    if r.runtime.graphics.needs_dxmt_runtime() {
        v.push(check(
            "DXMT",
            p.dxmt_runtime,
            Fail,
            yes_no(
                p.dxmt_runtime,
                &format!("{} u runtime-u", dxmt::DXMT_VERSION),
                "nije instaliran",
            ),
            &setup,
        ));
    }
    let Some(prefix) = &p.bottle else {
        v.push(check("Bottle", false, Fail, "ne postoji".into(), &setup));
        return v;
    };
    let full = p.reg_keys >= bottle::MIN_REG_KEYS;
    v.push(check(
        "Bottle",
        full,
        Fail,
        format!("{prefix} ({} kljuceva)", p.reg_keys),
        &format!("bw bottle repair {id}"),
    ));
    if r.runtime.graphics == Graphics::Dxmt && p.dxmt_runtime {
        v.push(check(
            "DXMT u bottle-u",
            p.dxmt_bottle,
            Warn,
            yes_no(
                p.dxmt_bottle,
                "system32/syswow64 ok",
                "DLL-ovi fale ili su stari",
            ),
            &run,
        ));
    }
    if r.runtime.wine == WineSource::Winehq {
        v.push(check(
            "GPTK ostaci",
            p.stale_dlls.is_empty(),
            Warn,
            if p.stale_dlls.is_empty() {
                "nema".into()
            } else {
                p.stale_dlls.join(", ")
            },
            &run,
        ));
    }
    if r.game.store != Store::Steam {
        return v;
    }
    v.push(check(
        "Steam",
        p.steam,
        Fail,
        yes_no(p.steam, "instaliran", "nije instaliran"),
        &setup,
    ));
    if !p.steam {
        return v;
    }
    if r.steam.cef_workaround == CefWorkaround::Wine {
        v.push(check(
            "steamwebhelper",
            !p.helper_wrapped,
            Warn,
            yes_no(
                !p.helper_wrapped,
                "original (crta ga nas Wine patch)",
                "stari bw wrapper (`bw run` vraca original)",
            ),
            &run,
        ));
    }
    if let Some(app) = r.game.steam_app_id {
        v.push(check(
            "Igra",
            p.game_installed,
            Warn,
            yes_no(
                p.game_installed,
                &format!("instalirana (Steam {app})"),
                "nije instalirana",
            ),
            &format!("bw run {id}, pa Install u Steam-u"),
        ));
    }
    v
}

fn print(checks: &[Check]) -> bool {
    let mut failed = false;
    for c in checks {
        let tag = match c.status {
            Status::Ok => "ok  ",
            Status::Warn => "upoz",
            Status::Fail => {
                failed = true;
                "FALI"
            }
        };
        println!("[{tag}] {:<15} {}", c.name, c.detail);
        if let Some(fix) = &c.fix {
            println!("       -> {fix}");
        }
    }
    failed
}

/// `bw doctor [igra]`: Mac, pa (ako je data igra) runtime, bottle i Steam za nju.
pub fn run(game: Option<&Recipe>) -> ExitCode {
    let mut failed = print(&evaluate(&probe()));
    if let Some(r) = game {
        println!("\n{} ({})", r.game.name, r.game.id);
        failed |= print(&evaluate_game(r, &probe_game(r)));
    }
    if failed {
        println!("\nNeke stvari fale. Popravi ih pa pokreni `bw doctor` ponovo.");
        ExitCode::FAILURE
    } else {
        println!("\nSve potrebno je tu.");
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m5_ready() -> Probe {
        Probe {
            os: "macos".into(),
            arch: "arm64".into(),
            macos_version: Some("26.1".into()),
            rosetta: true,
            xcode_clt: true,
            brew_arm: true,
            gptk_wine: Some(
                "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64".into(),
            ),
            ..Default::default()
        }
    }

    fn status(checks: &[Check], name: &str) -> Status {
        checks.iter().find(|c| c.name == name).unwrap().status
    }

    #[test]
    fn ready_m5_has_no_failures() {
        let c = evaluate(&m5_ready());
        assert!(c.iter().all(|c| c.status != Status::Fail), "{c:?}");
        assert_eq!(status(&c, "D3DMetal"), Status::Warn);
    }

    #[test]
    fn missing_rosetta_fails_with_fix() {
        let c = evaluate(&Probe {
            rosetta: false,
            ..m5_ready()
        });
        let r = c.iter().find(|c| c.name == "Rosetta 2").unwrap();
        assert_eq!(r.status, Status::Fail);
        assert!(r.fix.as_deref().unwrap().contains("install-rosetta"));
    }

    #[test]
    fn old_macos_fails() {
        let c = evaluate(&Probe {
            macos_version: Some("13.6".into()),
            ..m5_ready()
        });
        assert_eq!(status(&c, "macOS"), Status::Fail);
    }

    #[test]
    fn unwritable_homebrew_fails() {
        let c = evaluate(&Probe {
            brew_not_writable: true,
            ..m5_ready()
        });
        assert_eq!(status(&c, "Homebrew prava"), Status::Fail);
        assert!(evaluate(&m5_ready())
            .iter()
            .all(|c| c.name != "Homebrew prava"));
    }

    #[test]
    fn no_wine_fails() {
        let c = evaluate(&Probe {
            gptk_wine: None,
            ..m5_ready()
        });
        assert_eq!(status(&c, "Wine (GPTK)"), Status::Fail);
    }

    #[test]
    fn linux_stops_early() {
        let c = evaluate(&Probe {
            os: "linux".into(),
            ..Default::default()
        });
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].status, Status::Fail);
    }

    fn aoe3() -> Recipe {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("recipes/aoe3-de.toml");
        crate::recipe::load_file(&p).unwrap()
    }

    fn aoe3_ready() -> GameProbe {
        GameProbe {
            wine: Some("/h/runtimes/wine-devel-11.21.app/Contents/Resources/wine/bin/wine".into()),
            patched_modules: true,
            dxmt_runtime: true,
            bottle: Some("/h/bottles/aoe3-de".into()),
            reg_keys: 5000,
            dxmt_bottle: true,
            steam: true,
            helper_wrapped: false,
            game_installed: true,
            ..Default::default()
        }
    }

    #[test]
    fn ready_aoe3_is_all_ok() {
        let c = evaluate_game(&aoe3(), &aoe3_ready());
        assert!(c.iter().all(|c| c.status == Status::Ok), "{c:?}");
        let names: Vec<_> = c.iter().map(|c| c.name).collect();
        assert_eq!(
            names,
            [
                "Wine",
                "Wine moduli",
                "DXMT",
                "Bottle",
                "GPTK ostaci",
                "Steam",
                "steamwebhelper",
                "Igra"
            ]
        );
    }

    #[test]
    fn fresh_mac_points_to_setup() {
        let c = evaluate_game(&aoe3(), &GameProbe::default());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].status, Status::Fail);
        assert_eq!(c[0].fix.as_deref(), Some("bw setup aoe3-de"));
    }

    #[test]
    fn missing_pieces_fail_or_warn() {
        let p = GameProbe {
            quarantined: true,
            patched_modules: false,
            stale_dlls: vec!["atidxx64.dll".into()],
            helper_wrapped: true,
            game_installed: false,
            ..aoe3_ready()
        };
        let c = evaluate_game(&aoe3(), &p);
        assert_eq!(status(&c, "Karantin"), Status::Fail);
        assert!(c
            .iter()
            .find(|c| c.name == "Karantin")
            .unwrap()
            .fix
            .as_deref()
            .unwrap()
            .contains("xattr -dr com.apple.quarantine \"/h/runtimes/wine-devel-11.21.app\""));
        assert_eq!(status(&c, "Wine moduli"), Status::Fail);
        assert_eq!(status(&c, "GPTK ostaci"), Status::Warn);
        assert_eq!(status(&c, "steamwebhelper"), Status::Warn);
        assert_eq!(status(&c, "Igra"), Status::Warn);
    }

    #[test]
    fn no_bottle_stops_before_steam() {
        let c = evaluate_game(
            &aoe3(),
            &GameProbe {
                bottle: None,
                ..aoe3_ready()
            },
        );
        assert_eq!(status(&c, "Bottle"), Status::Fail);
        assert!(c.iter().all(|c| c.name != "Steam"));
    }
}
