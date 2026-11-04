//! Eksperimentalni native arm64 runtime: Wine bez Rosette, x86_64 kod prevodi FEX
//! u istom procesu (ARM64EC, `libarm64ecfex.dll`), kao na Windows on ARM.
//!
//! Build: Irz-Z/wine-macos-native v0.1.0 (Wine 11.10 arm64, LGPL; FEX-2609, MIT).
//! Za sada samo za mjerenje (bench/fex-exceptions): Rosetta trosi ~27 us po
//! #UD izuzetku iz AoE3 zastite, ovde ide kroz native arm64 trap.
//!
//! Njihov `libarm64ecfex.dll` mijenjamo nasim (patches/fex-2609, CI workflow
//! fex-arm64ec.yml): FEX-ov call-ret stek je sa 4K zastitnim stranicama koje na
//! macOS-u (16K) ne rade, pa se program zaglavi posle ~196k izuzetaka.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::bottle::{bottle_path, download_invocations, home, run_all, Invocation};
use crate::runtime::runtimes_dir;

pub const VERSION: &str = "0.1.0";
pub const URL: &str = "https://github.com/Irz-Z/wine-macos-native/releases/download/v0.1.0/wine-macos-native-arm64-v0.1.0.tar.gz";
pub const SHA256: &str = "355d5fc366c35c71b5a3e1ed95f6d208fd1f13ac12ae03ad5880a9186c63c5cb";
/// Odvojen prefiks: arm64 system32 se ne moze mijesati sa x86_64 bottle-ovima.
pub const BOTTLE: &str = "arm64-fex";

pub const FEX_PATCHES: [(&str, &str); 5] = [
    (
        "0001-load-teb-from-tsd-slot-and-wow64-guest-base.patch",
        include_str!("../patches/fex-2609/0001-load-teb-from-tsd-slot-and-wow64-guest-base.patch"),
    ),
    (
        "0003-callret-stack-guard-whole-host-pages.patch",
        include_str!("../patches/fex-2609/0003-callret-stack-guard-whole-host-pages.patch"),
    ),
    (
        "0004-reset-callret-stack-on-guest-exception.patch",
        include_str!("../patches/fex-2609/0004-reset-callret-stack-on-guest-exception.patch"),
    ),
    (
        "llvm-mingw-0001-ntcurrentteb-tsd.patch",
        include_str!("../patches/fex-2609/llvm-mingw-0001-ntcurrentteb-tsd.patch"),
    ),
    (
        "vixl-0002-include-map.patch",
        include_str!("../patches/fex-2609/vixl-0002-include-map.patch"),
    ),
];

/// Tag release-a sa nasim FEX DLL-om; mijenja se sa patch-evima (kao wine_build).
pub fn fex_tag() -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (name, text) in FEX_PATCHES {
        for b in name.bytes().chain([0]).chain(text.bytes()).chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("fex-2609-bw-{:012x}", h >> 16)
}

pub fn fex_dll_url() -> String {
    format!(
        "https://github.com/{}/releases/download/{}/libarm64ecfex.dll",
        crate::wine_build::RELEASE_REPO,
        fex_tag()
    )
}

fn fex_dll(home: &Path) -> PathBuf {
    runtime_dir(home).join("lib/wine/aarch64-windows/libarm64ecfex.dll")
}

fn fex_marker(home: &Path) -> PathBuf {
    fex_dll(home).with_extension("dll.bw-tag")
}

/// Skini nas FEX DLL i stavi ga umjesto originala (original ostaje kao .orig).
pub fn fex_invocations(home: &Path) -> Vec<Invocation> {
    let dll = fex_dll(home).display().to_string();
    let cached = home
        .join("cache")
        .join(format!("libarm64ecfex-{}.dll", fex_tag()))
        .display()
        .to_string();
    let mut steps = download_invocations(&fex_dll_url(), Path::new(&cached));
    let swap = format!(
        "[ -e '{dll}.orig' ] || cp '{dll}' '{dll}.orig'; cp '{cached}' '{dll}' && echo {} > '{}'",
        fex_tag(),
        fex_marker(home).display()
    );
    steps.push(inv("sh", &["-c", &swap], &[]));
    steps
}

pub fn runtime_dir(home: &Path) -> PathBuf {
    runtimes_dir(home).join(format!("wine-arm64-fex-{VERSION}"))
}

pub fn wine_bin(home: &Path) -> PathBuf {
    runtime_dir(home).join("bin/wine")
}

/// Bez WINEARM64ECEMU wineserver odbija AMD64 exe.
pub fn env(home: &Path) -> Vec<(String, String)> {
    vec![
        (
            "WINEPREFIX".into(),
            bottle_path(home, BOTTLE).display().to_string(),
        ),
        ("WINEARM64ECEMU".into(), "1".into()),
    ]
}

fn inv(program: &str, args: &[&str], env: &[(String, String)]) -> Invocation {
    Invocation {
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        env: env.iter().cloned().collect(),
    }
}

/// curl -> sha256 -> tar -> preimenuj -> skini karantin.
pub fn install_invocations(home: &Path) -> Vec<Invocation> {
    let tarball = home
        .join("cache")
        .join(format!("wine-macos-native-arm64-v{VERSION}.tar.gz"));
    let staging = runtimes_dir(home).join(format!(".unpack-arm64-{VERSION}"));
    let dest = runtime_dir(home);
    let (tarball, staging, dest) = (
        tarball.display().to_string(),
        staging.display().to_string(),
        dest.display().to_string(),
    );
    let mut steps = download_invocations(URL, Path::new(&tarball));
    let check = format!("echo '{SHA256}  {tarball}' | shasum -a 256 -c -");
    steps.extend([
        inv("sh", &["-c", &check], &[]),
        inv("mkdir", &["-p", &staging], &[]),
        inv("tar", &["-xzf", &tarball, "-C", &staging], &[]),
        inv("mv", &[&format!("{staging}/install"), &dest], &[]),
        inv("rm", &["-rf", &staging], &[]),
        inv("xattr", &["-dr", "com.apple.quarantine", &dest], &[]),
    ]);
    steps
}

/// Prefiks + FEX kao AMD64 emulator (HKLM\Software\Microsoft\Wow64\amd64).
pub fn prefix_invocations(home: &Path) -> Vec<Invocation> {
    let wine = wine_bin(home).display().to_string();
    let server = runtime_dir(home)
        .join("bin/wineserver")
        .display()
        .to_string();
    let env = env(home);
    vec![
        inv(&wine, &["wineboot", "-i"], &env),
        inv(
            &wine,
            &[
                "reg",
                "add",
                r"HKLM\Software\Microsoft\Wow64\amd64",
                "/ve",
                "/d",
                "libarm64ecfex.dll",
                "/f",
            ],
            &env,
        ),
        inv(&server, &["-w"], &env),
    ]
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

fn install(dry_run: bool) -> Result<(), String> {
    if !cfg!(target_os = "macos") && !dry_run {
        return Err("arm64 runtime je samo za Apple Silicon Mac".into());
    }
    let home = home();
    let mut steps = Vec::new();
    if !wine_bin(&home).exists() {
        if !dry_run {
            let _ = fs::create_dir_all(home.join("cache"));
            let _ = fs::create_dir_all(runtimes_dir(&home));
        }
        steps = install_invocations(&home);
        let tarball = home
            .join("cache")
            .join(format!("wine-macos-native-arm64-v{VERSION}.tar.gz"));
        if tarball.exists() {
            steps.drain(..2);
        }
    }
    let tag = fex_tag();
    if fs::read_to_string(fex_marker(&home))
        .map(|t| t.trim() != tag)
        .unwrap_or(true)
    {
        steps.extend(fex_invocations(&home));
    }
    if !bottle_path(&home, BOTTLE).join("drive_c").exists() {
        steps.extend(prefix_invocations(&home));
    }
    run_all(&steps, dry_run)?;
    let env: Vec<String> = env(&home)
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    println!(
        "arm64 Wine {VERSION} + FEX {tag}: {} {} <program.exe>",
        env.join(" "),
        wine_bin(&home).display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_verifies_checksum_before_unpacking() {
        let inv = install_invocations(Path::new("/h"));
        let progs: Vec<_> = inv.iter().map(|i| i.program.as_str()).collect();
        assert_eq!(
            progs,
            ["curl", "mv", "sh", "mkdir", "tar", "mv", "rm", "xattr"]
        );
        assert!(inv[2].args[1].contains(SHA256));
        assert_eq!(
            inv[5].args,
            [
                "/h/runtimes/.unpack-arm64-0.1.0/install",
                "/h/runtimes/wine-arm64-fex-0.1.0"
            ]
        );
    }

    #[test]
    fn fex_dll_is_swapped_and_original_kept() {
        let inv = fex_invocations(Path::new("/h"));
        assert_eq!(inv[0].program, "curl");
        assert!(inv[0].args.contains(&fex_dll_url()));
        assert!(fex_tag().starts_with("fex-2609-bw-"));
        let swap = &inv[2].args[1];
        assert!(swap.contains("libarm64ecfex.dll.orig"), "{swap}");
        assert!(swap.contains(&fex_tag()), "{swap}");
    }

    #[test]
    fn prefix_registers_fex_as_amd64_emulator() {
        let inv = prefix_invocations(Path::new("/h"));
        assert_eq!(inv[0].program, "/h/runtimes/wine-arm64-fex-0.1.0/bin/wine");
        assert!(inv[1].args.contains(&"libarm64ecfex.dll".to_string()));
        assert!(inv.iter().all(
            |i| i.env["WINEPREFIX"] == "/h/bottles/arm64-fex" && i.env["WINEARM64ECEMU"] == "1"
        ));
    }
}
