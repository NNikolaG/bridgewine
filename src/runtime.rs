//! Wine runtime-ovi koje BridgeWine sam drzi u `$BW_HOME/runtimes`.
//!
//! WineHQ build (Gcenx/macOS_Wine_builds, besplatan, otvoren kod): Homebrew cask-ovi
//! `wine-stable`/`wine@devel` su iskljuceni (Gatekeeper), pa skidamo tarball direktno.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::bottle::{download_invocations, home, run_all, Invocation};

pub const WINEHQ_VERSION: &str = "11.21";

pub fn winehq_url(version: &str) -> String {
    format!(
        "https://github.com/Gcenx/macOS_Wine_builds/releases/download/{version}/wine-devel-{version}-osx64.tar.xz"
    )
}

pub fn runtimes_dir(home: &Path) -> PathBuf {
    home.join("runtimes")
}

fn app_name(version: &str) -> String {
    format!("wine-devel-{version}.app")
}

fn wine_bin(app: &Path) -> PathBuf {
    app.join("Contents/Resources/wine/bin/wine")
}

/// "wine-devel-11.21.app" -> [11, 21]
fn parse_version(name: &str) -> Option<Vec<u32>> {
    let v = name.strip_prefix("wine-devel-")?.strip_suffix(".app")?;
    v.split('.').map(|p| p.parse().ok()).collect()
}

/// Najnoviji instalirani WineHQ `wine` u `dir`.
pub fn winehq_wine_in(dir: &Path) -> Option<String> {
    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(ver) = parse_version(&name) else {
            continue;
        };
        let bin = wine_bin(&entry.path());
        if bin.exists() && best.as_ref().is_none_or(|(b, _)| ver > *b) {
            best = Some((ver, bin));
        }
    }
    best.map(|(_, p)| p.display().to_string())
}

pub fn winehq_wine() -> Option<String> {
    winehq_wine_in(&runtimes_dir(&home()))
}

fn inv(program: &str, args: &[&str]) -> Invocation {
    Invocation {
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        env: Default::default(),
    }
}

/// curl -> tar -> preimenuj -> skini karantin (inace Gatekeeper ubija wine).
pub fn install_invocations(home: &Path, version: &str) -> Vec<Invocation> {
    let rt = runtimes_dir(home);
    let tarball = home
        .join("cache")
        .join(format!("wine-devel-{version}-osx64.tar.xz"));
    let staging = rt.join(format!(".unpack-{version}"));
    let app = rt.join(app_name(version));
    let (tarball, staging, app) = (
        tarball.display().to_string(),
        staging.display().to_string(),
        app.display().to_string(),
    );
    let unpacked = format!("{staging}/Wine Devel.app");
    let mut steps = download_invocations(&winehq_url(version), Path::new(&tarball));
    steps.extend([
        inv("mkdir", &["-p", &staging]),
        inv("tar", &["-xJf", &tarball, "-C", &staging]),
        inv("mv", &[&unpacked, &app]),
        inv("rmdir", &[&staging]),
        inv("xattr", &["-dr", "com.apple.quarantine", &app]),
    ]);
    steps
}

pub fn cmd_install(dry_run: bool) -> ExitCode {
    match install(dry_run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("bw: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Gde ce `wine` biti posle `bw runtime install` (za --dry-run pre instalacije).
pub fn expected_wine(home: &Path) -> String {
    wine_bin(&runtimes_dir(home).join(app_name(WINEHQ_VERSION)))
        .display()
        .to_string()
}

pub fn install(dry_run: bool) -> Result<(), String> {
    let home = home();
    let app = runtimes_dir(&home).join(app_name(WINEHQ_VERSION));
    if wine_bin(&app).exists() {
        println!("WineHQ {WINEHQ_VERSION} vec instaliran: {}", app.display());
        return Ok(());
    }
    if !dry_run {
        let _ = fs::create_dir_all(home.join("cache"));
    }
    let mut steps = install_invocations(&home, WINEHQ_VERSION);
    // tarball iz prethodnog pokusaja ne skidamo ponovo
    let tarball = home
        .join("cache")
        .join(format!("wine-devel-{WINEHQ_VERSION}-osx64.tar.xz"));
    if tarball.exists() {
        steps.drain(..2);
    }
    run_all(&steps, dry_run)?;
    println!("WineHQ {WINEHQ_VERSION}: {}", wine_bin(&app).display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_newest_installed_winehq() {
        let dir = std::env::temp_dir().join(format!("bw-rt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for v in ["11.9", "11.21", "11.3"] {
            let bin = wine_bin(&dir.join(app_name(v)));
            fs::create_dir_all(bin.parent().unwrap()).unwrap();
            fs::write(&bin, "").unwrap();
        }
        // nepotpun build se preskace
        fs::create_dir_all(dir.join(app_name("12.0"))).unwrap();
        let got = winehq_wine_in(&dir).unwrap();
        assert!(got.contains("wine-devel-11.21.app"), "{got}");
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(winehq_wine_in(&dir), None);
    }

    #[test]
    fn install_downloads_unpacks_and_clears_quarantine() {
        let inv = install_invocations(Path::new("/h"), "11.21");
        let progs: Vec<_> = inv.iter().map(|i| i.program.as_str()).collect();
        assert_eq!(
            progs,
            ["curl", "mv", "mkdir", "tar", "mv", "rmdir", "xattr"]
        );
        assert!(inv[0].args.contains(&winehq_url("11.21")));
        assert_eq!(
            inv[4].args,
            [
                "/h/runtimes/.unpack-11.21/Wine Devel.app",
                "/h/runtimes/wine-devel-11.21.app"
            ]
        );
        assert_eq!(
            inv[6].args,
            [
                "-dr",
                "com.apple.quarantine",
                "/h/runtimes/wine-devel-11.21.app"
            ]
        );
    }
}
