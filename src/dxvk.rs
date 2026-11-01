//! DXVK-macOS (github.com/Gcenx/DXVK-macOS): D3D10/D3D11 preko Vulkan-a (MoltenVK),
//! za `graphics = "dxvk"` ili `bw run <igra> --graphics dxvk`.
//!
//! AoE3 DE preko Metal prevodilaca (DXMT i D3DMetal) crta naslove i dugmad u meniju
//! samo delimicno (vidi se gornja petina slova, levi deo dugmeta); preko DXVK-a je
//! meni ispravan (AppleGamingWiki, CrossOver forum). Za razliku od DXMT-a, DXVK ne
//! menja runtime: DLL-ovi idu samo u bottle, a override vazi samo za exe igre, pa
//! Steam i njegov CEF ostaju na builtin dxgi/d3d11.
//!
//! Uzimamo pun release (sa `dxgi.dll`), ne "repack": DXVK-ov d3d11 trazi DXVK-ov
//! dxgi adapter, a obican WineHQ dxgi ga nema (Wine bug 58265).

use std::fs;
use std::path::{Path, PathBuf};

use crate::bottle::{download_invocations, home, run_all, Invocation};

pub const DXVK_VERSION: &str = "v1.10.3-20230507";

/// DLL-ovi koje DXVK zamenjuje.
pub const DLLS: [&str; 3] = ["d3d11", "d3d10core", "dxgi"];

/// (folder u release-u, folder u bottle-u)
const ARCHES: [(&str, &str); 2] = [("x64", "system32"), ("x32", "syswow64")];

fn name() -> String {
    format!("dxvk-macOS-async-{DXVK_VERSION}")
}

pub fn url() -> String {
    format!(
        "https://github.com/Gcenx/DXVK-macOS/releases/download/{DXVK_VERSION}/{}.tar.gz",
        name()
    )
}

/// Gde `ensure` raspakuje release.
pub fn unpacked_dir() -> PathBuf {
    home().join("cache").join(name()).join(name())
}

/// Kopira DXVK DLL-ove u system32/syswow64 bottle-a gde se razlikuju.
pub fn install_bottle(unpacked: &Path, prefix: &Path) -> Result<(), String> {
    for (arch, sys) in ARCHES {
        for dll in DLLS {
            let file = format!("{dll}.dll");
            let src = unpacked.join(arch).join(&file);
            let dst = prefix.join("drive_c/windows").join(sys).join(&file);
            let want = fs::read(&src).map_err(|e| format!("{}: {e}", src.display()))?;
            if fs::read(&dst).is_ok_and(|have| have == want) {
                continue;
            }
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&dst, want).map_err(|e| format!("{}: {e}", dst.display()))?;
        }
    }
    Ok(())
}

/// `WINEDLLOVERRIDES` deo za igre koje se ne pokrecu preko Steam-a.
pub fn env_overrides() -> String {
    format!("{}=n,b", DLLS.join(","))
}

/// .reg koji postavlja (DXVK) ili brise (ostalo) override samo za `exe`.
///
/// Steam pokrece igru kao svoje dete, pa `WINEDLLOVERRIDES` iz `bw run` vazi i za
/// Steam; zato per-app kljuc `AppDefaults\<exe>\DllOverrides`.
pub fn app_overrides_reg(exe: &str, dxvk: bool) -> String {
    let key = format!("HKEY_CURRENT_USER\\Software\\Wine\\AppDefaults\\{exe}\\DllOverrides");
    let mut s = format!("REGEDIT4\r\n\r\n[-{key}]\r\n");
    if dxvk {
        s.push_str(&format!("\r\n[{key}]\r\n"));
        for dll in DLLS {
            s.push_str(&format!("\"{dll}\"=\"native,builtin\"\r\n"));
        }
    }
    s
}

/// Skini i raspakuj DXVK u cache ako vec nije.
pub fn ensure(dry_run: bool) -> Result<PathBuf, String> {
    let unpacked = unpacked_dir();
    if unpacked.join("x64/d3d11.dll").exists() {
        return Ok(unpacked);
    }
    let cache = home().join("cache");
    let tarball = cache.join(format!("{}.tar.gz", name()));
    let dest = cache.join(name());
    let mut steps = Vec::new();
    if !tarball.exists() {
        steps.extend(download_invocations(&url(), &tarball));
    }
    let inv = |program: &str, args: Vec<String>| Invocation {
        program: program.into(),
        args,
        env: Default::default(),
    };
    steps.push(inv("mkdir", vec!["-p".into(), dest.display().to_string()]));
    steps.push(inv(
        "tar",
        vec![
            "-xzf".into(),
            tarball.display().to_string(),
            "-C".into(),
            dest.display().to_string(),
        ],
    ));
    if !dry_run {
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    }
    run_all(&steps, dry_run)?;
    if !dry_run {
        println!("DXVK {DXVK_VERSION} raspakovan u {}", unpacked.display());
    }
    Ok(unpacked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bw-dxvk-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn url_points_at_full_release_with_dxgi() {
        assert_eq!(
            url(),
            "https://github.com/Gcenx/DXVK-macOS/releases/download/v1.10.3-20230507/dxvk-macOS-async-v1.10.3-20230507.tar.gz"
        );
    }

    #[test]
    fn copies_both_arches_into_bottle() {
        let rel = tmp("rel");
        let prefix = tmp("bottle");
        for (arch, _) in ARCHES {
            fs::create_dir_all(rel.join(arch)).unwrap();
            for dll in DLLS {
                fs::write(
                    rel.join(arch).join(format!("{dll}.dll")),
                    format!("{arch} {dll}"),
                )
                .unwrap();
            }
        }
        let sys32 = prefix.join("drive_c/windows/system32");
        fs::create_dir_all(&sys32).unwrap();
        fs::write(sys32.join("dxgi.dll"), "dxmt dxgi").unwrap();

        install_bottle(&rel, &prefix).unwrap();
        assert_eq!(
            fs::read_to_string(sys32.join("dxgi.dll")).unwrap(),
            "x64 dxgi"
        );
        let wow = prefix.join("drive_c/windows/syswow64/d3d11.dll");
        assert_eq!(fs::read_to_string(wow).unwrap(), "x32 d3d11");
        fs::remove_dir_all(&rel).unwrap();
        fs::remove_dir_all(&prefix).unwrap();
    }

    #[test]
    fn overrides_only_the_game_exe() {
        let on = app_overrides_reg("AoE3DE_s.exe", true);
        assert!(on.starts_with("REGEDIT4\r\n"));
        assert!(on.contains(
            "[-HKEY_CURRENT_USER\\Software\\Wine\\AppDefaults\\AoE3DE_s.exe\\DllOverrides]"
        ));
        assert!(on.contains("\"dxgi\"=\"native,builtin\""));
        let off = app_overrides_reg("AoE3DE_s.exe", false);
        assert!(off.contains("[-HKEY"));
        assert!(!off.contains("native"));
        assert_eq!(env_overrides(), "d3d11,d3d10core,dxgi=n,b");
    }
}
