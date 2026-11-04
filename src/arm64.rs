//! Eksperimentalni native arm64 runtime: Wine bez Rosette, x86_64 kod prevodi FEX
//! u istom procesu (ARM64EC, `libarm64ecfex.dll`), kao na Windows on ARM.
//!
//! Build: Irz-Z/wine-macos-native v0.1.0 (Wine 11.10 arm64, LGPL; FEX-2609, MIT).
//! Za sada samo za mjerenje (bench/fex-exceptions): Rosetta trosi ~27 us po
//! #UD izuzetku iz AoE3 zastite, ovde ide kroz native arm64 trap.

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
    if !bottle_path(&home, BOTTLE).join("drive_c").exists() {
        steps.extend(prefix_invocations(&home));
    }
    run_all(&steps, dry_run)?;
    let env: Vec<String> = env(&home)
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    println!(
        "arm64 Wine + FEX {VERSION}: {} {} <program.exe>",
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
    fn prefix_registers_fex_as_amd64_emulator() {
        let inv = prefix_invocations(Path::new("/h"));
        assert_eq!(inv[0].program, "/h/runtimes/wine-arm64-fex-0.1.0/bin/wine");
        assert!(inv[1].args.contains(&"libarm64ecfex.dll".to_string()));
        assert!(inv.iter().all(
            |i| i.env["WINEPREFIX"] == "/h/bottles/arm64-fex" && i.env["WINEARM64ECEMU"] == "1"
        ));
    }
}
