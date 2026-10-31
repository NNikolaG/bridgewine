//! `bw doctor`: proverava da li Mac ima sve sto treba za BridgeWine.

use std::path::Path;
use std::process::{Command, ExitCode};

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
    pub fix: Option<&'static str>,
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
    std::env::var("BW_WINE")
        .ok()
        .filter(|p| Path::new(p).exists())
        .or_else(|| first_existing(GPTK_WINE_PATHS))
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
        gptk_wine: std::env::var("BW_WINE")
            .ok()
            .filter(|p| Path::new(p).exists())
            .or_else(|| first_existing(GPTK_WINE_PATHS)),
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
            Some("GPTK i D3DMetal traze macOS 14 Sonoma ili noviji"),
        ),
        None => (Warn, Some("ne mogu da procitam verziju (sw_vers)")),
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
        fix: (!apple).then_some("BridgeWine cilja M-serija cipove; na Intel Macu D3DMetal ne radi"),
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
            fix: (!p.rosetta).then_some("softwareupdate --install-rosetta --agree-to-license"),
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
        fix: (!p.xcode_clt).then_some("xcode-select --install"),
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
        fix: (!p.brew_arm && !p.brew_x86).then_some("https://brew.sh"),
    });
    if p.brew_not_writable {
        v.push(Check {
            name: "Homebrew prava",
            status: Fail,
            detail: "/opt/homebrew nije upisiv za tvog korisnika".into(),
            fix: Some("sudo chown -R $(whoami) /opt/homebrew  (nikad `sudo brew`)"),
        });
    }

    v.push(Check {
        name: "Wine (GPTK)",
        status: if p.gptk_wine.is_some() { Ok } else { Fail },
        detail: p.gptk_wine.clone().unwrap_or_else(|| "nije pronadjen".into()),
        fix: p.gptk_wine.is_none().then_some(
            "brew install --cask gcenx/wine/game-porting-toolkit, pa xattr -dr com.apple.quarantine \"/Applications/Game Porting Toolkit.app\" (docs/install-gptk.md); ili postavi BW_WINE",
        ),
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
            .then_some("potreban samo za DX12 i recepte sa graphics = \"d3dmetal\""),
    });
    v
}

pub fn run() -> ExitCode {
    let checks = evaluate(&probe());
    let mut failed = false;
    for c in &checks {
        let tag = match c.status {
            Status::Ok => "ok  ",
            Status::Warn => "upoz",
            Status::Fail => {
                failed = true;
                "FALI"
            }
        };
        println!("[{tag}] {:<14} {}", c.name, c.detail);
        if let Some(fix) = c.fix {
            println!("       -> {fix}");
        }
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
        assert!(r.fix.unwrap().contains("install-rosetta"));
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
}
