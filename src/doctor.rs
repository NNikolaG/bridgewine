//! `bw doctor`: proverava da li Mac ima sve sto treba za BridgeWine.

use std::path::Path;
use std::process::{Command, ExitCode};

use crate::recipe::WineSource;
use crate::runtime;

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
    /// WineHQ build iz `$BW_HOME/runtimes` (za 64-bitni Steam).
    pub winehq_wine: Option<String>,
    pub d3dmetal: Option<String>,
    /// Interfejs default rute (npr. "en0"), iz `route -n get default`.
    pub net_iface: Option<String>,
    /// Uredjaj Wi-Fi porta (npr. "en0"), iz `networksetup -listallhardwareports`.
    pub wifi_iface: Option<String>,
    /// awdl0 (AirDrop/Handoff) je UP i aktivan: pravi ping skokove na Wi-Fi-ju.
    pub awdl_active: bool,
}

/// "interface: en0" iz `route -n get default`.
pub fn parse_route_iface(out: &str) -> Option<String> {
    out.lines()
        .filter_map(|l| l.trim().strip_prefix("interface:"))
        .map(|i| i.trim().to_string())
        .find(|i| !i.is_empty())
}

/// Device posle "Hardware Port: Wi-Fi" u `networksetup -listallhardwareports`.
pub fn parse_wifi_iface(out: &str) -> Option<String> {
    let mut lines = out.lines().map(str::trim);
    while let Some(l) = lines.next() {
        if matches!(l, "Hardware Port: Wi-Fi" | "Hardware Port: AirPort") {
            return lines
                .next()
                .and_then(|d| d.strip_prefix("Device:"))
                .map(|d| d.trim().to_string());
        }
    }
    None
}

/// `ifconfig awdl0`: aktivan ako ima UP flag i "status: active".
pub fn parse_awdl_active(out: &str) -> bool {
    let up = out
        .lines()
        .next()
        .and_then(|l| l.split_once('<'))
        .and_then(|(_, r)| r.split_once('>'))
        .is_some_and(|(flags, _)| flags.split(',').any(|f| f == "UP"));
    up && out.lines().any(|l| l.trim() == "status: active")
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
        net_iface: if mac {
            cmd_out("route", &["-n", "get", "default"]).and_then(|o| parse_route_iface(&o))
        } else {
            None
        },
        wifi_iface: if mac {
            cmd_out("networksetup", &["-listallhardwareports"]).and_then(|o| parse_wifi_iface(&o))
        } else {
            None
        },
        awdl_active: mac && cmd_out("ifconfig", &["awdl0"]).is_some_and(|o| parse_awdl_active(&o)),
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
        name: "Wine (WineHQ)",
        status: if p.winehq_wine.is_some() { Ok } else { Warn },
        detail: p
            .winehq_wine
            .clone()
            .unwrap_or_else(|| "nije instaliran (treba za recepte sa wine = \"winehq\")".into()),
        fix: p.winehq_wine.is_none().then_some("bw runtime install"),
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

    // Mreza: Wine ne dodaje ping, ali Wi-Fi + AWDL na Mac-u pravi skokove u online igrama.
    let on_wifi = p.net_iface.is_some() && p.net_iface == p.wifi_iface;
    v.push(Check {
        name: "Mreza",
        status: if p.net_iface.is_some() && !on_wifi {
            Ok
        } else {
            Warn
        },
        detail: match &p.net_iface {
            None => "nema default rute (offline?)".into(),
            Some(i) if on_wifi => format!("Wi-Fi ({i})"),
            Some(i) => format!("kabl/drugo ({i})"),
        },
        fix: on_wifi.then_some("za online igre (CS2 i sl.) kabl daje stabilniji ping"),
    });
    if on_wifi && p.awdl_active {
        v.push(Check {
            name: "AWDL",
            status: Warn,
            detail: "awdl0 aktivan (AirDrop/Handoff): ping skokovi svakih par sekundi".into(),
            fix: Some(
                "dok igras iskljuci AirDrop i Handoff, ili `sudo ifconfig awdl0 down` (vraca se posle restarta)",
            ),
        });
    }
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
            net_iface: Some("en5".into()),
            wifi_iface: Some("en0".into()),
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
    fn wifi_with_awdl_warns() {
        let wifi = Probe {
            net_iface: Some("en0".into()),
            awdl_active: true,
            ..m5_ready()
        };
        let c = evaluate(&wifi);
        assert_eq!(status(&c, "Mreza"), Status::Warn);
        assert_eq!(status(&c, "AWDL"), Status::Warn);
        // Na kablu AWDL ne smeta, pa ga ne prijavljujemo.
        let c = evaluate(&Probe {
            awdl_active: true,
            ..m5_ready()
        });
        assert_eq!(status(&c, "Mreza"), Status::Ok);
        assert!(c.iter().all(|c| c.name != "AWDL"));
    }

    #[test]
    fn parses_macos_network_output() {
        let route = "   route to: default\ndestination: default\n    gateway: 192.168.1.1\n  interface: en0\n      flags: <UP,GATEWAY,DONE,STATIC,PRCLONING,GLOBAL>\n";
        assert_eq!(parse_route_iface(route).as_deref(), Some("en0"));
        let ports = "Hardware Port: Thunderbolt Bridge\nDevice: bridge0\nEthernet Address: N/A\n\nHardware Port: Wi-Fi\nDevice: en0\nEthernet Address: aa:bb:cc:dd:ee:ff\n";
        assert_eq!(parse_wifi_iface(ports).as_deref(), Some("en0"));
        let awdl = "awdl0: flags=8943<UP,BROADCAST,RUNNING,PROMISC,SIMPLEX,MULTICAST> mtu 1484\n\tstatus: active\n";
        assert!(parse_awdl_active(awdl));
        let down =
            "awdl0: flags=8902<BROADCAST,PROMISC,SIMPLEX,MULTICAST> mtu 1484\n\tstatus: inactive\n";
        assert!(!parse_awdl_active(down));
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
