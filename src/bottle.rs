//! Bottle = jedan Wine prefiks po igri, u `$BW_HOME/bottles/<id>`
//! (podrazumevano `~/Games/BridgeWine/bottles/<id>`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::doctor::find_wine_for;
use crate::dxmt;
use crate::dxvk;
use crate::recipe::{CefWorkaround, Graphics, Recipe, Store, WineSource};
use crate::runtime;
use crate::wine_build;

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

/// MoltenVK ICD koji WineHQ bundle nosi u `share/vulkan/icd.d`.
///
/// Gcenx-ov ntdll.so sam dodaje ovaj folder Vulkan loader-u; vanilla Wine (nas
/// ntdll.so iz `bw runtime build-patched`) ne, pa winevulkan nema drajver i
/// Steam-ov CEF (ANGLE preko Vulkan-a) beskonacno pokusava da napravi GL kontekst.
fn vulkan_env(wine: &str) -> Option<(String, String)> {
    let icd = Path::new(wine)
        .parent()?
        .parent()?
        .join("share/vulkan/icd.d/MoltenVK_icd.json");
    icd.exists()
        .then(|| ("VK_ADD_DRIVER_FILES".into(), icd.display().to_string()))
}

pub fn steam_install_invocation(wine: &str, prefix: &Path, setup: &Path) -> Invocation {
    let mut env = base_env(prefix);
    env.extend(vulkan_env(wine));
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

/// Nas wrapper (wrapper/steamwebhelper-wrapper.c), izgradjen sa
/// `x86_64-w64-mingw32-gcc -O2 -s -municode -mwindows`.
pub const HELPER_WRAPPER: &[u8] = include_bytes!("../wrapper/steamwebhelper.exe");
const HELPER: &str = "steamwebhelper.exe";
const HELPER_REAL: &str = "steamwebhelper_real.exe";

/// Steam-ov CEF helper crta u posebnom GPU procesu, a winemac.drv taj izlaz ne
/// prikazuje (Wine bug 56014), pa je prozor crn. `-cef-*` flagovi steam.exe-a ne
/// stizu do helpera, zato helper menjamo wrapperom koji pokrece pravi helper sa
/// `--disable-gpu`. Steam pri azuriranju vraca original, pa ovo radimo pri svakom startu.
pub const STEAM_ARGS: [&str; 3] = ["-no-cef-sandbox", "-cef-single-process", "-noverifyfiles"];
const STEAM_DLL_OVERRIDES: &str = "gameoverlayrenderer,gameoverlayrenderer64=d";

fn steam_dir(prefix: &Path) -> PathBuf {
    prefix
        .join("drive_c")
        .join("Program Files (x86)")
        .join("Steam")
}

/// Bilo koja verzija naseg wrappera: jedino on u sebi ima ime pravog helpera (UTF-16).
pub fn is_helper_wrapper(exe: &[u8]) -> bool {
    let needle: Vec<u8> = HELPER_REAL
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    exe.windows(needle.len()).any(|w| w == needle)
}

/// Vraca true ako je helper upravo (ponovo) umotan. Bez Steam-ovog CEF foldera ne radi nista.
///
/// Wrapper se nikad ne preimenuje u `_real` (inace pokrece sam sebe u krug); ako je
/// `_real` nestao ili je i on wrapper, javljamo gresku umesto da pogadjamo.
pub fn wrap_steam_helper(prefix: &Path) -> Result<bool, String> {
    let cef = steam_dir(prefix).join("bin").join("cef").join("cef.win64");
    let helper = cef.join(HELPER);
    let real = cef.join(HELPER_REAL);
    let Ok(current) = fs::read(&helper) else {
        return Ok(false);
    };
    let err = |e: std::io::Error| format!("{}: {e}", helper.display());
    if !is_helper_wrapper(&current) {
        // original (npr. posle Steam update-a) postaje _real
        fs::rename(&helper, &real).map_err(err)?;
    } else if !fs::read(&real).is_ok_and(|b| !is_helper_wrapper(&b)) {
        return Err(format!(
            "{} nedostaje ili je wrapper; vrati originalni Steam helper (npr. obrisi oba fajla i pokreni Steam bez -noverifyfiles)",
            real.display()
        ));
    } else if current == HELPER_WRAPPER {
        return Ok(false);
    }
    fs::write(&helper, HELPER_WRAPPER).map_err(err)?;
    Ok(true)
}

/// Brise Chromium `Singleton*` lockove koje ostavi Steam koji je pao; sa njima novi
/// helper misli da vec radi (dve Steam ikone u Dock-u, prazan prozor).
pub fn clear_cef_locks(prefix: &Path) {
    let Ok(users) = fs::read_dir(prefix.join("drive_c").join("users")) else {
        return;
    };
    for user in users.flatten() {
        let cache = user.path().join("AppData/Local/Steam/htmlcache");
        let Ok(entries) = fs::read_dir(&cache) else {
            continue;
        };
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("Singleton") {
                let _ = fs::remove_file(e.path());
            }
        }
    }
}

const RUN_KEY: &str = r"[Software\\Microsoft\\Windows\\CurrentVersion\\Run]";

/// Izbacuje `"Steam"` iz HKCU `...\CurrentVersion\Run`. Wine na startu sesije sam
/// pokrece `steam.exe -silent` bez nasih flagova, i to pre nego sto bi `reg delete`
/// stigao, zato menjamo `user.reg` direktno (vazi kad wineserver ne radi).
pub fn strip_steam_autostart(user_reg: &str) -> Option<String> {
    let mut in_run = false;
    let mut changed = false;
    let mut out = String::with_capacity(user_reg.len());
    for line in user_reg.split_inclusive('\n') {
        if line.starts_with('[') {
            in_run = line.starts_with(RUN_KEY);
        } else if in_run && line.to_ascii_lowercase().starts_with("\"steam\"=") {
            changed = true;
            continue;
        }
        out.push_str(line);
    }
    changed.then_some(out)
}

/// Da li je Windows Steam instaliran u bottle-u.
pub fn steam_installed(prefix: &Path) -> bool {
    steam_dir(prefix).join("steam.exe").is_file()
}

/// Da li je helper trenutno nas wrapper (Steam ga vraca pri azuriranju; `bw run` ponovo umota).
pub fn steam_helper_wrapped(prefix: &Path) -> bool {
    let helper = steam_dir(prefix).join("bin/cef/cef.win64").join(HELPER);
    fs::read(helper).is_ok_and(|b| is_helper_wrapper(&b))
}

/// Da li je igra instalirana u Steam biblioteci bottle-a (podrazumevana biblioteka).
pub fn steam_game_installed(prefix: &Path, app_id: u32) -> bool {
    steam_dir(prefix)
        .join("steamapps")
        .join(format!("appmanifest_{app_id}.acf"))
        .is_file()
}

fn prep_steam(prefix: &Path, cef: CefWorkaround) -> Result<(), String> {
    if cef == CefWorkaround::Wrapper && wrap_steam_helper(prefix)? {
        println!("steamwebhelper umotan (BridgeWine wrapper)");
    }
    clear_cef_locks(prefix);
    let reg = prefix.join("user.reg");
    if let Some(t) = fs::read_to_string(&reg)
        .ok()
        .and_then(|t| strip_steam_autostart(&t))
    {
        fs::write(&reg, t).map_err(|e| format!("{}: {e}", reg.display()))?;
    }
    Ok(())
}

/// Builtin DLL starog Wine ABI-ja: uvozi `ntdll.__wine_unix_call`, koji WineHQ 9+ vise
/// ne izvozi (sada `__wine_unix_call_dispatcher`). Takve ostavi GPTK (D3DMetal-ov
/// `atidxx64.dll`, `nvapi64.dll`, `nvngx.dll`...) kad bottle predje na WineHQ;
/// AMD AGS u igri ucita `atidxx64.dll` i proces pukne ("unimplemented function
/// ntdll.dll.__wine_unix_call").
pub fn is_stale_builtin(pe: &[u8]) -> bool {
    fn has(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }
    pe.starts_with(b"MZ") && has(pe, b"Wine builtin DLL") && has(pe, b"__wine_unix_call\0")
}

/// Zastareli builtin DLL-ovi u system32/syswow64: (podfolder, putanja).
pub fn stale_builtins(prefix: &Path) -> Vec<(&'static str, PathBuf)> {
    let mut found = Vec::new();
    for sub in ["system32", "syswow64"] {
        let dir = prefix.join("drive_c").join("windows").join(sub);
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            let is_dll = path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("dll"));
            if is_dll && fs::read(&path).is_ok_and(|b| is_stale_builtin(&b)) {
                found.push((sub, path));
            }
        }
    }
    found
}

/// Sklanja zastarele builtin DLL-ove iz system32/syswow64 u `<bottle>/bw-stale-dlls/`.
pub fn quarantine_stale_builtins(prefix: &Path) -> Result<Vec<PathBuf>, String> {
    let mut moved = Vec::new();
    for (sub, path) in stale_builtins(prefix) {
        let dest_dir = prefix.join("bw-stale-dlls").join(sub);
        fs::create_dir_all(&dest_dir).map_err(|e| format!("{}: {e}", dest_dir.display()))?;
        let Some(name) = path.file_name() else {
            continue;
        };
        let dest = dest_dir.join(name);
        fs::rename(&path, &dest).map_err(|e| format!("{}: {e}", path.display()))?;
        moved.push(dest);
    }
    Ok(moved)
}

fn prep_winehq(wine: &str, prefix: &Path, patched: bool) -> Result<(), String> {
    for p in quarantine_stale_builtins(prefix)? {
        println!("sklonjen zastareli GPTK DLL: {}", p.display());
    }
    if !patched {
        return Ok(());
    }
    if let Some(app) = wine_build::app_for(wine) {
        if !wine_build::is_installed(&app) {
            eprintln!(
                "upozorenje: runtime nema BridgeWine Wine module (igre sa zastitom padaju); pokreni `bw runtime install-patched` (ili `bw runtime build-patched`)"
            );
        }
    }
    Ok(())
}

fn prep_session(wine: &str, prefix: &Path, dry_run: bool) -> Result<(), String> {
    run_all(&session_prep_invocations(wine, prefix), dry_run)
}

pub fn run_invocation(wine: &str, prefix: &Path, recipe: &Recipe) -> Result<Invocation, String> {
    let mut env = base_env(prefix);
    env.extend(vulkan_env(wine));
    env.extend(recipe.env.clone());
    let mut overrides = recipe.dll_overrides_string();
    if recipe.game.store == Store::Steam {
        if !overrides.is_empty() {
            overrides.push(';');
        }
        overrides.push_str(STEAM_DLL_OVERRIDES);
    }
    if !overrides.is_empty() {
        env.insert("WINEDLLOVERRIDES".into(), overrides);
    }
    let args = match (
        recipe.game.store,
        recipe.game.steam_app_id,
        &recipe.game.exe,
    ) {
        (Store::Steam, Some(app), _) => {
            let mut a = vec![STEAM_EXE.to_string()];
            a.extend(STEAM_ARGS.map(String::from));
            a.extend(["-applaunch".into(), app.to_string()]);
            a
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

/// Preuzimanje u `<dest>.part`, pa `mv` u `dest`: prekinut ili neuspeo curl ne ostavlja
/// polovican fajl koji bi sledeci put bio prihvacen kao gotov (proveravamo samo `exists()`).
pub(crate) fn download_invocations(url: &str, dest: &Path) -> Vec<Invocation> {
    let dest = dest.display().to_string();
    let part = format!("{dest}.part");
    let inv = |program: &str, args: &[&str]| Invocation {
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        env: BTreeMap::new(),
    };
    vec![
        inv("curl", &["-fL", "-o", &part, url]),
        inv("mv", &["-f", &part, &dest]),
    ]
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

/// Wine za komandu; sa --dry-run pre `bw runtime install` vraca putanju gde ce biti.
pub(crate) fn wine_for(recipe: &Recipe, dry_run: bool) -> Result<String, String> {
    match wine_or_err(recipe) {
        Err(_) if dry_run && recipe.runtime.wine == WineSource::Winehq => {
            Ok(runtime::expected_wine(&home()))
        }
        r => r,
    }
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
    finish(create(recipe, dry_run))
}

pub fn create(recipe: &Recipe, dry_run: bool) -> Result<(), String> {
    let wine = wine_for(recipe, dry_run)?;
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
}

pub fn cmd_steam_install(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish(steam_install(recipe, dry_run))
}

pub fn steam_install(recipe: &Recipe, dry_run: bool) -> Result<(), String> {
    let wine = wine_for(recipe, dry_run)?;
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
        if !dry_run {
            fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        }
        run_all(&download_invocations(STEAM_SETUP_URL, &setup), dry_run)
            .map_err(|e| format!("preuzimanje SteamSetup.exe nije uspelo: {e}"))?;
    }
    prep_session(&wine, &prefix, dry_run)?;
    run_all(&[steam_install_invocation(&wine, &prefix, &setup)], dry_run)?;
    println!(
        "Steam instaliran. Sledece: `bw run {}`, uloguj se i instaliraj igru u Steamu.",
        recipe.game.id
    );
    Ok(())
}

const GRAPHICS_REG_FILE: &str = "bridgewine-graphics.reg";
const GRAPHICS_REG_WIN: &str = r"C:\bridgewine-graphics.reg";

/// DXVK DLL-ovi u bottle i override samo za exe igre (ostali prevodioci ga brisu).
/// Vraca dodatak za `WINEDLLOVERRIDES` kad override ne moze per-app.
fn prep_graphics(
    wine: &str,
    prefix: &Path,
    recipe: &Recipe,
    graphics: Graphics,
    dry_run: bool,
) -> Result<Option<String>, String> {
    let dxvk = graphics == Graphics::Dxvk;
    if dxvk {
        let unpacked = dxvk::ensure(dry_run)?;
        if !dry_run {
            dxvk::install_bottle(&unpacked, prefix)?;
        }
    }
    let exe = recipe
        .game
        .exe
        .as_deref()
        .and_then(|e| e.rsplit(['\\', '/']).next());
    match (recipe.game.store, exe) {
        (Store::Steam, Some(exe)) => {
            if !dry_run {
                let drive_c = prefix.join("drive_c");
                fs::write(
                    drive_c.join(GRAPHICS_REG_FILE),
                    dxvk::app_overrides_reg(exe, dxvk),
                )
                .map_err(|e| format!("{}: {e}", drive_c.display()))?;
            }
            run_all(
                &[Invocation {
                    program: wine.into(),
                    args: vec!["regedit".into(), "/S".into(), GRAPHICS_REG_WIN.into()],
                    env: base_env(prefix),
                }],
                dry_run,
            )?;
            Ok(None)
        }
        _ if dxvk => Ok(Some(dxvk::env_overrides())),
        _ => Ok(None),
    }
}

/// `graphics` iz komandne linije ima prednost nad receptom.
pub fn cmd_run(recipe: &Recipe, graphics: Option<Graphics>, dry_run: bool) -> ExitCode {
    finish(run(recipe, graphics, dry_run))
}

pub fn run(recipe: &Recipe, graphics: Option<Graphics>, dry_run: bool) -> Result<(), String> {
    let graphics = graphics.unwrap_or(recipe.runtime.graphics);
    if !graphics.supports(recipe.runtime.directx) {
        return Err(format!(
            "graphics = \"{graphics}\" ne pokriva DirectX {}",
            recipe.runtime.directx
        ));
    }
    let wine = wine_for(recipe, dry_run)?;
    let prefix = bottle_path(&home(), &recipe.game.id);
    if !dry_run && !bottle_exists(&prefix) {
        return Err(format!(
            "bottle ne postoji, prvo `bw bottle create {}`",
            recipe.game.id
        ));
    }
    if graphics.needs_dxmt_runtime() {
        let app = wine_build::app_for(&wine).ok_or("ne znam gde je Wine runtime")?;
        dxmt::ensure(&app, dry_run)?;
        if graphics == Graphics::Dxmt && !dry_run {
            dxmt::sync_bottle(&app, &prefix)?;
        }
    }
    let mut inv = run_invocation(&wine, &prefix, recipe)?;
    if recipe.runtime.wine == WineSource::Winehq && !dry_run {
        prep_winehq(&wine, &prefix, recipe.runtime.patched)?;
    }
    if recipe.game.store == Store::Steam && !dry_run {
        prep_steam(&prefix, recipe.steam.cef_workaround)?;
    }
    prep_session(&wine, &prefix, dry_run)?;
    if let Some(extra) = prep_graphics(&wine, &prefix, recipe, graphics, dry_run)? {
        let o = inv.env.entry("WINEDLLOVERRIDES".into()).or_default();
        if !o.is_empty() {
            o.push(';');
        }
        o.push_str(&extra);
    }
    println!("grafika: {graphics}");
    run_all(&[inv], dry_run)
}

pub fn cmd_repair(recipe: &Recipe, dry_run: bool) -> ExitCode {
    finish((|| {
        let wine = wine_for(recipe, dry_run)?;
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
        let wine = wine_for(recipe, dry_run)?;
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
        assert_eq!(
            inv.args,
            [
                STEAM_EXE,
                "-no-cef-sandbox",
                "-cef-single-process",
                "-noverifyfiles",
                "-applaunch",
                "933110"
            ]
        );
        assert_eq!(inv.env["WINEMSYNC"], "1");
        assert_eq!(
            inv.env["WINEDLLOVERRIDES"],
            "dxgi=native,builtin;gameoverlayrenderer,gameoverlayrenderer64=d"
        );
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
    fn wraps_steam_helper_and_rewraps_after_update() {
        let dir = std::env::temp_dir().join(format!("bw-cef-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(!wrap_steam_helper(&dir).unwrap());
        let cef = steam_dir(&dir).join("bin/cef/cef.win64");
        fs::create_dir_all(&cef).unwrap();
        fs::write(cef.join(HELPER), b"orig1").unwrap();

        assert!(wrap_steam_helper(&dir).unwrap());
        assert_eq!(fs::read(cef.join(HELPER)).unwrap(), HELPER_WRAPPER);
        assert_eq!(fs::read(cef.join(HELPER_REAL)).unwrap(), b"orig1");
        assert!(!wrap_steam_helper(&dir).unwrap());

        // Steam update vraca svoj helper.
        fs::write(cef.join(HELPER), b"orig2").unwrap();
        assert!(wrap_steam_helper(&dir).unwrap());
        assert_eq!(fs::read(cef.join(HELPER)).unwrap(), HELPER_WRAPPER);
        assert_eq!(fs::read(cef.join(HELPER_REAL)).unwrap(), b"orig2");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn older_wrapper_is_replaced_not_renamed_to_real() {
        let dir = std::env::temp_dir().join(format!("bw-cef-old-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let cef = steam_dir(&dir).join("bin/cef/cef.win64");
        fs::create_dir_all(&cef).unwrap();
        let mut old_wrapper = b"MZ other build ".to_vec();
        old_wrapper.extend(HELPER_REAL.encode_utf16().flat_map(u16::to_le_bytes));
        fs::write(cef.join(HELPER), &old_wrapper).unwrap();
        fs::write(cef.join(HELPER_REAL), b"real helper").unwrap();

        assert!(wrap_steam_helper(&dir).unwrap());
        assert_eq!(fs::read(cef.join(HELPER)).unwrap(), HELPER_WRAPPER);
        assert_eq!(fs::read(cef.join(HELPER_REAL)).unwrap(), b"real helper");

        // _real je i sam wrapper: ne pogadjamo, javljamo gresku
        fs::write(cef.join(HELPER_REAL), &old_wrapper).unwrap();
        assert!(wrap_steam_helper(&dir).is_err());
        fs::remove_file(cef.join(HELPER_REAL)).unwrap();
        assert!(wrap_steam_helper(&dir).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shipped_wrapper_is_recognized() {
        assert!(is_helper_wrapper(HELPER_WRAPPER));
        assert!(!is_helper_wrapper(b"MZ steamwebhelper_real.exe in ASCII"));
    }

    #[test]
    fn wrapper_is_a_pe_executable() {
        assert_eq!(&HELPER_WRAPPER[..2], b"MZ");
    }

    #[test]
    fn clears_cef_singleton_locks() {
        let dir = std::env::temp_dir().join(format!("bw-lock-{}", std::process::id()));
        let cache = dir.join("drive_c/users/u/AppData/Local/Steam/htmlcache");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("SingletonLock"), "").unwrap();
        fs::write(cache.join("SingletonSocket"), "").unwrap();
        fs::write(cache.join("Cookies"), "").unwrap();
        clear_cef_locks(&dir);
        assert!(!cache.join("SingletonLock").exists());
        assert!(!cache.join("SingletonSocket").exists());
        assert!(cache.join("Cookies").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn strips_only_steam_from_run_key() {
        let reg = "WINE REGISTRY Version 2\n\
[Software\\\\Microsoft\\\\Windows\\\\CurrentVersion\\\\Run] 1793533771\n\
#time=1dd\n\
\"Steam\"=\"\\\"C:\\\\Program Files (x86)\\\\Steam\\\\steam.exe\\\" -silent\"\n\
\"Other\"=\"x.exe\"\n\
\n\
[Software\\\\Valve\\\\Steam] 1\n\
\"Steam\"=\"keep\"\n";
        let out = strip_steam_autostart(reg).unwrap();
        assert!(!out.contains("-silent"));
        assert!(out.contains("\"Other\"=\"x.exe\""));
        assert!(out.contains("\"Steam\"=\"keep\""));
        assert_eq!(strip_steam_autostart(&out), None);
    }

    #[test]
    fn download_goes_through_part_file() {
        let inv = download_invocations("https://x/y.exe", Path::new("/c/y.exe"));
        assert_eq!(inv[0].program, "curl");
        assert_eq!(
            inv[0].args,
            ["-fL", "-o", "/c/y.exe.part", "https://x/y.exe"]
        );
        assert_eq!(inv[1].program, "mv");
        assert_eq!(inv[1].args, ["-f", "/c/y.exe.part", "/c/y.exe"]);
    }

    #[test]
    fn points_vulkan_loader_at_bundled_moltenvk() {
        let dir = std::env::temp_dir().join(format!("bw-vk-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let wine = dir.join("bin/wine").display().to_string();
        assert_eq!(vulkan_env(&wine), None);
        let icd = dir.join("share/vulkan/icd.d/MoltenVK_icd.json");
        fs::create_dir_all(icd.parent().unwrap()).unwrap();
        fs::write(&icd, "{}").unwrap();
        let inv = run_invocation(&wine, Path::new("/p"), &recipe()).unwrap();
        assert_eq!(inv.env["VK_ADD_DRIVER_FILES"], icd.display().to_string());
        fs::remove_dir_all(&dir).unwrap();
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
