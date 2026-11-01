//! Recepti: jedan TOML fajl po igri u `recipes/<id>.toml`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub game: Game,
    pub runtime: Runtime,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub dll_overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub winetricks: Winetricks,
    #[serde(default)]
    pub steam: SteamOpts,
    #[serde(default)]
    pub known_issues: BTreeMap<String, String>,
    #[serde(default)]
    pub tested: Tested,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub store: Store,
    pub steam_app_id: Option<u32>,
    pub exe: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Store {
    Steam,
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runtime {
    /// Odakle dolazi Wine.
    pub wine: WineSource,
    /// Kako se DirectX prevodi u Metal.
    pub graphics: Graphics,
    pub directx: u8,
    #[serde(default = "default_windows_version")]
    pub windows_version: String,
    /// Igra trazi nase Wine module (`patches/`: direktni syscall-ovi, GLOBALROOT,
    /// Rosetta CPU frekvencija). `bw setup` ih skida sa release-a ili gradi.
    #[serde(default)]
    pub patched: bool,
}

/// Kako Steam-ov CEF (steamwebhelper) dobija sliku na macOS-u.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SteamOpts {
    #[serde(default)]
    pub cef_workaround: CefWorkaround,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum CefWorkaround {
    /// Nas kernelbase patch (0004) pokrece helper sa `--disable-gpu --single-process`
    /// (Wine bug 56014). Treba `patched = true`. `"wrapper"` je staro ime.
    #[default]
    #[serde(alias = "wrapper")]
    Wine,
    /// Bez zaobilazenja (npr. kad Wine sam ispravno prikazuje CEF).
    None,
}

fn default_windows_version() -> String {
    "win10".into()
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum WineSource {
    /// Appleov Game Porting Toolkit (Wine + D3DMetal).
    Gptk,
    /// Zvanicni WineHQ build (Gcenx/macOS_Wine_builds) u `$BW_HOME/runtimes`, `bw runtime install`.
    /// Potreban za 64-bitni Steam klijent (2026+): GPTK-ov Wine 7.7 nema BCrypt pseudo-handle.
    Winehq,
    /// Bilo koji drugi Wine (npr. iz Homebrewa), putanja preko BW_WINE.
    System,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Graphics {
    D3dmetal,
    Dxmt,
    Dxvk,
    Wined3d,
}

impl Graphics {
    /// Treba li DXMT u runtime-u. Za DXVK takodje: DXVK vazi samo za exe igre,
    /// a Steam i njegov CEF ostaju na runtime-ovom builtin d3d11/dxgi, sto je
    /// sa DXMT-om provereno na M5 (2026-11-01).
    pub fn needs_dxmt_runtime(self) -> bool {
        matches!(self, Graphics::Dxmt | Graphics::Dxvk)
    }

    /// DirectX verzije koje ovaj prevodilac pokriva.
    pub fn supports(self, dx: u8) -> bool {
        match self {
            Graphics::D3dmetal => matches!(dx, 11 | 12),
            Graphics::Dxmt => matches!(dx, 10 | 11),
            Graphics::Dxvk => matches!(dx, 8..=11),
            Graphics::Wined3d => matches!(dx, 7..=11),
        }
    }
}

impl fmt::Display for Graphics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Graphics::D3dmetal => "d3dmetal",
            Graphics::Dxmt => "dxmt",
            Graphics::Dxvk => "dxvk",
            Graphics::Wined3d => "wined3d",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Winetricks {
    #[serde(default)]
    pub packages: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tested {
    pub chip: Option<String>,
    pub macos: Option<String>,
    #[serde(default)]
    pub rating: Rating,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Platinum,
    Gold,
    Silver,
    Bronze,
    Borked,
    #[default]
    Untested,
}

const DLL_MODES: &[&str] = &[
    "native",
    "builtin",
    "native,builtin",
    "builtin,native",
    "disabled",
    "",
];

impl Recipe {
    pub fn parse(text: &str) -> Result<Recipe, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Provere koje TOML sema ne moze da uhvati.
    pub fn validate(&self, file_stem: &str) -> Vec<String> {
        let mut errs = Vec::new();
        let g = &self.game;
        if g.id != file_stem {
            errs.push(format!(
                "game.id '{}' se ne poklapa sa imenom fajla '{file_stem}'",
                g.id
            ));
        }
        if g.id.is_empty()
            || !g
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            errs.push(format!(
                "game.id '{}' sme da sadrzi samo a-z, 0-9 i '-'",
                g.id
            ));
        }
        if g.store == Store::Steam && g.steam_app_id.is_none() {
            errs.push("store = \"steam\" zahteva game.steam_app_id".into());
        }
        let rt = &self.runtime;
        if !rt.graphics.supports(rt.directx) {
            errs.push(format!(
                "graphics '{}' ne podrzava DirectX {}",
                rt.graphics, rt.directx
            ));
        }
        if rt.patched && rt.wine != WineSource::Winehq {
            errs.push("patched = true zahteva wine = \"winehq\"".into());
        }
        if rt.graphics == Graphics::D3dmetal && rt.wine != WineSource::Gptk {
            errs.push("d3dmetal zahteva wine = \"gptk\"".into());
        }
        for (dll, mode) in &self.dll_overrides {
            if !DLL_MODES.contains(&mode.as_str()) {
                errs.push(format!("dll_overrides.{dll}: nepoznat mod '{mode}'"));
            }
        }
        for key in self.env.keys() {
            if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                errs.push(format!("env: neispravno ime promenljive '{key}'"));
            }
        }
        errs
    }

    /// WINEDLLOVERRIDES string, npr. "d3d11=native,builtin;dxgi=native".
    pub fn dll_overrides_string(&self) -> String {
        self.dll_overrides
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(";")
    }
}

pub fn recipes_dir(flag: Option<PathBuf>) -> PathBuf {
    flag.or_else(|| std::env::var_os("BW_RECIPES_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("recipes"))
}

pub struct Loaded {
    pub path: PathBuf,
    pub result: Result<Recipe, Vec<String>>,
}

pub fn load_all(dir: &Path) -> Result<Vec<Loaded>, String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("ne mogu da otvorim {}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|path| Loaded {
            result: load_file(&path),
            path,
        })
        .collect())
}

pub fn load_file(path: &Path) -> Result<Recipe, Vec<String>> {
    let text = fs::read_to_string(path).map_err(|e| vec![e.to_string()])?;
    let recipe = Recipe::parse(&text).map_err(|e| vec![e])?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let errs = recipe.validate(stem);
    if errs.is_empty() {
        Ok(recipe)
    } else {
        Err(errs)
    }
}

pub fn cmd_list(dir: &Path) -> ExitCode {
    match load_all(dir) {
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
        Ok(all) => {
            for l in all {
                match l.result {
                    Ok(r) => println!(
                        "{:<16} {:<10} DX{:<3} {:?}  {}",
                        r.game.id,
                        r.runtime.graphics.to_string(),
                        r.runtime.directx,
                        r.tested.rating,
                        r.game.name
                    ),
                    Err(_) => println!(
                        "{:<16} (neispravan, pokreni `bw recipe check`)",
                        l.path.display()
                    ),
                }
            }
            ExitCode::SUCCESS
        }
    }
}

pub fn cmd_show(dir: &Path, id: &str) -> ExitCode {
    let path = dir.join(format!("{id}.toml"));
    match load_file(&path) {
        Ok(r) => {
            println!("{} ({})", r.game.name, r.game.id);
            if let Some(app) = r.game.steam_app_id {
                println!("  Steam app id:  {app}");
            }
            if let Some(exe) = &r.game.exe {
                println!("  Exe:           {exe}");
            }
            println!("  Wine:          {:?}", r.runtime.wine);
            println!(
                "  Grafika:       {} (DirectX {})",
                r.runtime.graphics, r.runtime.directx
            );
            println!("  Windows:       {}", r.runtime.windows_version);
            if r.runtime.patched {
                println!("  Wine moduli:   BridgeWine patch-evi (patches/)");
            }
            if r.game.store == Store::Steam {
                println!("  Steam CEF:     {:?}", r.steam.cef_workaround);
            }
            for (k, v) in &r.env {
                println!("  env {k}={v}");
            }
            if !r.dll_overrides.is_empty() {
                println!("  WINEDLLOVERRIDES={}", r.dll_overrides_string());
            }
            if !r.winetricks.packages.is_empty() {
                println!("  winetricks:    {}", r.winetricks.packages.join(" "));
            }
            for (k, v) in &r.known_issues {
                println!("  poznato [{k}]: {v}");
            }
            let on = [
                r.tested.chip.as_deref(),
                r.tested.macos.as_deref().map(|m| m.trim()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", macOS ");
            if on.is_empty() {
                println!("  Ocena:         {:?}", r.tested.rating);
            } else {
                println!("  Ocena:         {:?} ({on})", r.tested.rating);
            }
            ExitCode::SUCCESS
        }
        Err(errs) => {
            eprintln!("{}:", path.display());
            for e in errs {
                eprintln!("  {e}");
            }
            ExitCode::FAILURE
        }
    }
}

pub fn cmd_check(dir: &Path) -> ExitCode {
    let all = match load_all(dir) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let mut bad = 0;
    for l in &all {
        match &l.result {
            Ok(_) => println!("ok    {}", l.path.display()),
            Err(errs) => {
                bad += 1;
                println!("GRESKA {}", l.path.display());
                for e in errs {
                    println!("        {e}");
                }
            }
        }
    }
    println!("{} recepata, {} sa greskom", all.len(), bad);
    if bad == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[game]
id = "aoe3-de"
name = "Age of Empires III: Definitive Edition"
store = "steam"
steam_app_id = 933110

[runtime]
wine = "gptk"
graphics = "dxvk"
directx = 11

[dll_overrides]
dxgi = "native,builtin"
d3d11 = "native,builtin"
"#;

    #[test]
    fn parses_and_validates_good_recipe() {
        let r = Recipe::parse(GOOD).unwrap();
        assert!(r.validate("aoe3-de").is_empty());
        assert_eq!(r.runtime.windows_version, "win10");
        assert_eq!(
            r.dll_overrides_string(),
            "d3d11=native,builtin;dxgi=native,builtin"
        );
    }

    #[test]
    fn id_must_match_file_name() {
        let r = Recipe::parse(GOOD).unwrap();
        assert!(r
            .validate("other")
            .iter()
            .any(|e| e.contains("imenom fajla")));
    }

    #[test]
    fn d3dmetal_does_not_do_dx9() {
        let txt = GOOD
            .replace("graphics = \"dxvk\"", "graphics = \"d3dmetal\"")
            .replace("directx = 11", "directx = 9");
        let r = Recipe::parse(&txt).unwrap();
        assert!(r
            .validate("aoe3-de")
            .iter()
            .any(|e| e.contains("DirectX 9")));
    }

    #[test]
    fn steam_requires_app_id() {
        let r = Recipe::parse(&GOOD.replace("steam_app_id = 933110\n", "")).unwrap();
        assert!(r
            .validate("aoe3-de")
            .iter()
            .any(|e| e.contains("steam_app_id")));
    }

    #[test]
    fn crossover_is_not_a_wine_source() {
        assert!(Recipe::parse(&GOOD.replace("wine = \"gptk\"", "wine = \"crossover\"")).is_err());
    }

    #[test]
    fn unknown_field_is_rejected() {
        assert!(
            Recipe::parse(&GOOD.replace("directx = 11", "directx = 11\nengine = \"x\"")).is_err()
        );
    }

    #[test]
    fn steam_cef_workaround_defaults_to_wine() {
        let r = Recipe::parse(GOOD).unwrap();
        assert_eq!(r.steam.cef_workaround, CefWorkaround::Wine);
        assert!(!r.runtime.patched);
        let r = Recipe::parse(&format!("{GOOD}\n[steam]\ncef_workaround = \"none\"\n")).unwrap();
        assert_eq!(r.steam.cef_workaround, CefWorkaround::None);
        let r = Recipe::parse(&format!("{GOOD}\n[steam]\ncef_workaround = \"wrapper\"\n")).unwrap();
        assert_eq!(r.steam.cef_workaround, CefWorkaround::Wine);
    }

    #[test]
    fn patched_needs_winehq() {
        let r =
            Recipe::parse(&GOOD.replace("directx = 11", "directx = 11\npatched = true")).unwrap();
        assert!(r.validate("aoe3-de").iter().any(|e| e.contains("patched")));
        let ok = GOOD
            .replace("wine = \"gptk\"", "wine = \"winehq\"")
            .replace("directx = 11", "directx = 11\npatched = true");
        assert!(Recipe::parse(&ok).unwrap().validate("aoe3-de").is_empty());
    }

    #[test]
    fn bundled_recipes_are_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("recipes");
        for l in load_all(&dir).unwrap() {
            assert!(
                l.result.is_ok(),
                "{}: {:?}",
                l.path.display(),
                l.result.err()
            );
        }
    }
}
