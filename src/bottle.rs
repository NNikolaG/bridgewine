//! Bottle = jedan Wine prefiks po igri, u `$BW_HOME/bottles/<id>`
//! (podrazumevano `~/Games/BridgeWine/bottles/<id>`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::doctor::find_wine_for;
use crate::recipe::{Graphics, Recipe, Store, WineSource};

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

/// Prefiks postoji cim wineboot napravi `drive_c`; `system.reg` se upisuje tek kad wineserver izadje.
pub fn bottle_exists(prefix: &Path) -> bool {
    prefix.join("drive_c").is_dir()
}

/// `wineserver` je pored `wine64` u istom bin folderu.
pub fn wineserver_for(wine: &str) -> String {
    match Path::new(wine).parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join("wineserver").display().to_string(),
        _ => "wineserver".into(),
    }
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
    if !s.is_empty()
        && s.chars()
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

fn wineserver(wine: &str, prefix: &Path, flag: &str) -> Invocation {
    Invocation {
        program: wineserver_for(wine),
        args: vec![flag.into()],
        env: base_env(prefix),
    }
}

/// Puna inicijalizacija prefiksa: `wineboot -u` registruje builtin DLL-ove
/// (kripto provajdere, nsiproxy...), a `wineserver -w` ceka da se sve upise.
/// Bez toga Steam pada sa "http error 0" jer HTTPS ne radi.
pub fn init_invocations(wine: &str, prefix: &Path) -> Vec<Invocation> {
    vec![
        Invocation {
            program: wine.into(),
            args: vec!["wineboot".into(), "-u".into()],
            env: base_env(prefix),
        },
        Invocation {
            program: wine.into(),
            args: vec!["regedit".into(), "/S".into(), NOPROXY_REG_WIN.into()],
            env: base_env(prefix),
        },
        wineserver(wine, prefix, "-w"),
    ]
}

/// Iskljucuje "Automatically detect settings" u IE podesavanjima (wininet magic 0x46,
/// flags = 1, direktno). Samo ovo NE sprecava Steam-ov WPAD upit: Steam zove
/// `WinHttpGetProxyForUrl` sa auto-detekcijom bez obzira na ovo; to resava
/// `session_prep_invocations` (prazan DNS `Domain`).
pub const NOPROXY_REG: &str = "REGEDIT4\r\n\r\n\
[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\\Connections]\r\n\
\"DefaultConnectionSettings\"=hex:46,00,00,00,00,00,00,00,01,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00\r\n";
const NOPROXY_REG_FILE: &str = "bridgewine-noproxy.reg";
const NOPROXY_REG_WIN: &str = r"C:\bridgewine-noproxy.reg";

/// Upisuje .reg fajl u `drive_c` da bi ga Wine regedit video kao `C:\...`.
pub fn write_noproxy_reg(prefix: &Path) -> Result<(), String> {
    let drive_c = prefix.join("drive_c");
    fs::create_dir_all(&drive_c).map_err(|e| format!("{}: {e}", drive_c.display()))?;
    fs::write(drive_c.join(NOPROXY_REG_FILE), NOPROXY_REG).map_err(|e| e.to_string())
}

/// Ugasi sve Wine procese bottle-a, pa ga ponovo inicijalizuj.
pub fn repair_invocations(wine: &str, prefix: &Path) -> Vec<Invocation> {
    let mut v = vec![wineserver(wine, prefix, "-k")];
    v.extend(init_invocations(wine, prefix));
    v
}

/// Zdrav prefiks ima hiljade kljuceva u system.reg; polu-inicijalizovan ima desetine.
pub const MIN_REG_KEYS: usize = 1000;

pub fn registry_keys(prefix: &Path) -> usize {
    fs::read_to_string(prefix.join("system.reg"))
        .map(|t| t.lines().filter(|l| l.starts_with('[')).count())
        .unwrap_or(0)
}

/// Inicijalizacija, pa `winecfg -v` za verziju Windowsa iz recepta.
pub fn create_invocations(wine: &str, prefix: &Path, recipe: &Recipe) -> Vec<Invocation> {
    let env = base_env(prefix);
    let mut v = init_invocations(wine, prefix);
    v.extend([
        Invocation {
            program: wine.into(),
            args: vec![
                "winecfg".into(),
                "-v".into(),
                recipe.runtime.windows_version.clone(),
            ],
            env,
        },
        wineserver(wine, prefix, "-w"),
    ]);
    v
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

const TCPIP_PARAMS: &str = r"HKLM\System\CurrentControlSet\Services\Tcpip\Parameters";

/// Priprema Wine sesije pre Steam-a.
///
/// Steam updater (WinHTTP) uvek radi WPAD auto-proxy detekciju: `wpad.<Domain>` kao
/// DNS-only upit (`DnsQuery`), mimo /etc/hosts i mDNS-a. `Domain` Wine uzima iz macOS
/// hostname-a (`MacBook.local` -> `local`), a mnogi DNS serveri (npr. ISP) `.local` upite
/// tiho odbacuju, pa svaki upit visi ~135 s i Steam javi "needs to be online to update".
/// Sa praznim `Domain` FQDN nema tacku i WinHTTP preskace DNS deo detekcije.
///
/// wineboot pri svakom startu sesije ponovo upisuje `Domain`, zato: `wineserver -p30`
/// drzi sesiju zivom, `wineboot` zavrsi svoje, pa tek onda brisemo `Domain`.
pub fn session_prep_invocations(wine: &str, prefix: &Path) -> Vec<Invocation> {
    let env = base_env(prefix);
    vec![
        Invocation {
            program: wineserver_for(wine),
            args: vec!["-p30".into()],
            env: env.clone(),
        },
        Invocation {
            program: wine.into(),
            args: vec!["wineboot".into()],
            env: env.clone(),
        },
        Invocation {
            program: wine.into(),
            args: [
                "reg",
                "add",
                TCPIP_PARAMS,
                "/v",
                "Domain",
                "/t",
                "REG_SZ",
                "/d",
                "",
                "/f",
            ]
            .map(String::from)
            .to_vec(),
            env,
        },
    ]
}

fn prep_session(wine: &str, prefix: &Path, dry_run: bool) -> Result<(), String> {
    run_all(&session_prep_invocations(wine, prefix), dry_run)
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

pub(crate) fn run_all(invs: &[Invocation], dry_run: bool) -> Result<(), String> {
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
            if may_fail(inv) {
                continue;
            }
            return Err(format!("komanda nije uspela ({status})"));
        }
    }
    Ok(())
}

/// `wineserver -k` bez pokrenute sesije i `wineserver -p` uz vec pokrenutu vracaju 1; to nije greska.
fn may_fail(inv: &Invocation) -> bool {
    inv.program.ends_with("wineserver")
        && inv
            .args
            .first()
            .is_some_and(|a| a == "-k" || a.starts_with("-p"))
}

fn wine_or_err(recipe: &Recipe) -> Result<String, String> {
    let src = recipe.runtime.wine;
    find_wine_for(src).ok_or_else(|| match src {
        WineSource::Winehq => "WineHQ Wine nije instaliran; pokreni `bw runtime install`".into(),
        WineSource::Gptk => "Wine (GPTK) nije pronadjen; pokreni `bw doctor`".into(),
        WineSource::System => "wine = \"system\" trazi BW_WINE=<putanja do wine>".into(),
    })
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
        let wine = wine_or_err(recipe)?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if bottle_exists(&prefix) {
            println!("bottle vec postoji: {}", prefix.display());
            return Ok(());
        }
        if !dry_run {
            fs::create_dir_all(&prefix).map_err(|e| format!("{}: {e}", prefix.display()))?;
        }
        if !dry_run {
            write_noproxy_reg(&prefix)?;
        }
        run_all(&create_invocations(&wine, &prefix, recipe), dry_run)?;
        println!("bottle spreman: {}", prefix.display());
        Ok(())
    })())
}

pub fn cmd_steam_install(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_or_err(recipe)?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if !dry_run && !bottle_exists(&prefix) {
            return Err(format!(
                "bottle ne postoji, prvo `bw bottle create {}`",
                recipe.game.id
            ));
        }
        if !dry_run && registry_keys(&prefix) < MIN_REG_KEYS {
            println!(
                "bottle nije potpuno inicijalizovan ({} kljuceva u registry-ju), popravljam...",
                registry_keys(&prefix)
            );
            write_noproxy_reg(&prefix)?;
            run_all(&repair_invocations(&wine, &prefix), false)?;
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
        prep_session(&wine, &prefix, dry_run)?;
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
        let wine = wine_or_err(recipe)?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if !dry_run && !bottle_exists(&prefix) {
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
        prep_session(&wine, &prefix, dry_run)?;
        run_all(&[inv], dry_run)
    })())
}

pub fn cmd_repair(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_or_err(recipe)?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        if !dry_run && !bottle_exists(&prefix) {
            return Err(format!(
                "bottle ne postoji, prvo `bw bottle create {}`",
                recipe.game.id
            ));
        }
        if !dry_run {
            write_noproxy_reg(&prefix)?;
        }
        run_all(&repair_invocations(&wine, &prefix), dry_run)?;
        if !dry_run {
            println!("registry: {} kljuceva", registry_keys(&prefix));
        }
        Ok(())
    })())
}

pub fn cmd_kill(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_or_err(recipe)?;
        let prefix = bottle_path(&home(), &recipe.game.id);
        run_all(&[wineserver(&wine, &prefix, "-k")], dry_run)
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
    fn bottle_exists_once_drive_c_is_there() {
        let dir = std::env::temp_dir().join(format!("bw-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(!bottle_exists(&dir));
        fs::create_dir_all(dir.join("drive_c")).unwrap();
        assert!(bottle_exists(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn wineserver_sits_next_to_wine() {
        assert_eq!(
            wineserver_for(
                "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64"
            ),
            "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wineserver"
        );
        assert_eq!(wineserver_for("wine64"), "wineserver");
    }

    #[test]
    fn repair_kills_then_reinits() {
        let inv = repair_invocations("/w/wine64", Path::new("/p"));
        let args: Vec<_> = inv.iter().map(|i| i.args.join(" ")).collect();
        assert_eq!(
            args,
            [
                "-k",
                "wineboot -u",
                r"regedit /S C:\bridgewine-noproxy.reg",
                "-w"
            ]
        );
    }

    #[test]
    fn counts_registry_keys() {
        let dir = std::env::temp_dir().join(format!("bw-reg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(registry_keys(&dir), 0);
        fs::write(
            dir.join("system.reg"),
            "WINE REGISTRY Version 2\n[A]\n\"x\"=1\n[B]\n",
        )
        .unwrap();
        assert_eq!(registry_keys(&dir), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn noproxy_reg_disables_autodetect() {
        assert!(NOPROXY_REG.starts_with("REGEDIT4"));
        assert!(NOPROXY_REG.contains(r"Internet Settings\Connections]"));
        // magic 0x46, flags 0x01 (PROXY_TYPE_DIRECT, bez AUTO_DETECT = 0x08)
        assert!(NOPROXY_REG.contains("hex:46,00,00,00,00,00,00,00,01,00,00,00"));
        let dir = std::env::temp_dir().join(format!("bw-noproxy-{}", std::process::id()));
        write_noproxy_reg(&dir).unwrap();
        assert!(dir.join("drive_c").join(NOPROXY_REG_FILE).exists());
        fs::remove_dir_all(&dir).unwrap();
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
        assert_eq!(inv[0].args, ["wineboot", "-u"]);
        assert_eq!(inv[1].args, ["regedit", "/S", NOPROXY_REG_WIN]);
        assert_eq!(inv[2].program, "/w/wineserver");
        assert_eq!(inv[2].args, ["-w"]);
        assert_eq!(inv[3].args, ["winecfg", "-v", "win10"]);
        assert_eq!(inv[4].args, ["-w"]);
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
    fn session_prep_clears_dns_domain_after_wineboot() {
        let inv = session_prep_invocations("/w/wine64", Path::new("/p"));
        assert_eq!(inv[0].program, "/w/wineserver");
        assert_eq!(inv[0].args, ["-p30"]);
        assert_eq!(inv[1].args, ["wineboot"]);
        assert_eq!(
            inv[2].args,
            [
                "reg",
                "add",
                TCPIP_PARAMS,
                "/v",
                "Domain",
                "/t",
                "REG_SZ",
                "/d",
                "",
                "/f"
            ]
        );
        assert!(inv.iter().all(|i| i.env["WINEPREFIX"] == "/p"));
    }

    #[test]
    fn killing_or_pinning_wineserver_may_fail() {
        assert!(may_fail(&wineserver("/w/wine", Path::new("/p"), "-k")));
        assert!(may_fail(
            &session_prep_invocations("/w/wine", Path::new("/p"))[0]
        ));
        assert!(!may_fail(&wineserver("/w/wine", Path::new("/p"), "-w")));
        assert!(!may_fail(
            &session_prep_invocations("/w/wine", Path::new("/p"))[1]
        ));
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
