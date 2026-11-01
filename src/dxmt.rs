//! DXMT (github.com/3Shain/dxmt): D3D11/D3D10 preko Metal-a, za `graphics = "dxmt"`.
//!
//! wined3d preko macOS OpenGL 4.1 ne nudi D3D11 feature level 11, pa AoE3 DE javi
//! "Your video hardware does not support DirectX 11". DXMT-ov "builtin" release
//! ide u runtime (`lib/wine/<arch>/`), jer Wine builtin DLL iz system32 svejedno
//! ucitava iz lib foldera. Wine-ovi originali ostaju pored kao `*.orig`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::bottle::{home, run_all, Invocation};

pub const DXMT_VERSION: &str = "v0.80";

/// (folder u release-u i u `lib/wine/`, fajl)
pub const FILES: [(&str, &str); 9] = [
    ("x86_64-windows", "d3d11.dll"),
    ("x86_64-windows", "dxgi.dll"),
    ("x86_64-windows", "d3d10core.dll"),
    ("x86_64-windows", "winemetal.dll"),
    ("i386-windows", "d3d11.dll"),
    ("i386-windows", "dxgi.dll"),
    ("i386-windows", "d3d10core.dll"),
    ("i386-windows", "winemetal.dll"),
    ("x86_64-unix", "winemetal.so"),
];

pub fn url(version: &str) -> String {
    format!(
        "https://github.com/3Shain/dxmt/releases/download/{version}/dxmt-{version}-builtin.tar.gz"
    )
}

fn lib_dir(app: &Path) -> PathBuf {
    app.join("Contents/Resources/wine/lib/wine")
}

fn stamp_path(app: &Path) -> PathBuf {
    lib_dir(app).join("bw-dxmt.txt")
}

fn stamp_text(version: &str, sizes: &[u64]) -> String {
    let mut s = format!("{version}\n");
    for ((dir, name), size) in FILES.iter().zip(sizes) {
        s.push_str(&format!("{dir}/{name} {size}\n"));
    }
    s
}

fn installed_sizes(app: &Path) -> Option<Vec<u64>> {
    FILES
        .iter()
        .map(|(dir, name)| {
            fs::metadata(lib_dir(app).join(dir).join(name))
                .ok()
                .map(|m| m.len())
        })
        .collect()
}

pub fn is_installed(app: &Path, version: &str) -> bool {
    let Some(sizes) = installed_sizes(app) else {
        return false;
    };
    fs::read_to_string(stamp_path(app)).is_ok_and(|s| s == stamp_text(version, &sizes))
}

/// Kopira raspakovan release (`<unpacked>/<dir>/<file>`) u runtime, cuvajuci originale.
pub fn install_files(unpacked: &Path, app: &Path, version: &str) -> Result<(), String> {
    let err = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    for (dir, name) in FILES {
        let src = unpacked.join(dir).join(name);
        let dst = lib_dir(app).join(dir).join(name);
        let orig = dst.with_file_name(format!("{name}.orig"));
        if dst.exists() && !orig.exists() {
            fs::copy(&dst, &orig).map_err(|e| err(&orig, e))?;
        }
        // preko privremenog fajla: procesi koji vec rade zadrze stari
        let tmp = dst.with_file_name(format!("{name}.bw-new"));
        fs::copy(&src, &tmp).map_err(|e| err(&src, e))?;
        fs::rename(&tmp, &dst).map_err(|e| err(&dst, e))?;
    }
    let sizes = installed_sizes(app).ok_or("DXMT fajl nedostaje posle instalacije")?;
    fs::write(stamp_path(app), stamp_text(version, &sizes)).map_err(|e| err(&stamp_path(app), e))
}

fn inv(program: &str, args: &[String]) -> Invocation {
    Invocation {
        program: program.into(),
        args: args.to_vec(),
        env: Default::default(),
    }
}

/// Skini (jednom, u cache) i instaliraj DXMT u runtime ako vec nije.
pub fn ensure(app: &Path, dry_run: bool) -> Result<(), String> {
    if is_installed(app, DXMT_VERSION) {
        return Ok(());
    }
    let cache = home().join("cache");
    let tarball = cache.join(format!("dxmt-{DXMT_VERSION}-builtin.tar.gz"));
    let unpack = cache.join(format!("dxmt-{DXMT_VERSION}"));
    let mut steps = Vec::new();
    if !tarball.exists() {
        steps.push(inv(
            "curl",
            &[
                "-fL".into(),
                "-o".into(),
                tarball.display().to_string(),
                url(DXMT_VERSION),
            ],
        ));
    }
    steps.push(inv("mkdir", &["-p".into(), unpack.display().to_string()]));
    steps.push(inv(
        "tar",
        &[
            "-xzf".into(),
            tarball.display().to_string(),
            "-C".into(),
            unpack.display().to_string(),
        ],
    ));
    if !dry_run {
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    }
    run_all(&steps, dry_run)?;
    if dry_run {
        return Ok(());
    }
    install_files(&unpack.join(DXMT_VERSION), app, DXMT_VERSION)?;
    println!("DXMT {DXMT_VERSION} instaliran u {}", app.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bw-dxmt-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn url_points_at_official_release() {
        assert_eq!(
            url("v0.80"),
            "https://github.com/3Shain/dxmt/releases/download/v0.80/dxmt-v0.80-builtin.tar.gz"
        );
    }

    #[test]
    fn installs_over_wine_dlls_and_keeps_originals() {
        let rel = tmp("rel");
        let app = tmp("app");
        for (dir, name) in FILES {
            fs::create_dir_all(rel.join(dir)).unwrap();
            fs::write(rel.join(dir).join(name), format!("dxmt {name}")).unwrap();
            fs::create_dir_all(lib_dir(&app).join(dir)).unwrap();
        }
        // Wine ima svoj d3d11, a winemetal nema
        let wine_d3d11 = lib_dir(&app).join("x86_64-windows/d3d11.dll");
        fs::write(&wine_d3d11, "wine d3d11").unwrap();
        assert!(!is_installed(&app, "v0.80"));

        install_files(&rel, &app, "v0.80").unwrap();
        assert!(is_installed(&app, "v0.80"));
        assert!(!is_installed(&app, "v0.81"));
        assert_eq!(fs::read_to_string(&wine_d3d11).unwrap(), "dxmt d3d11.dll");
        let orig = lib_dir(&app).join("x86_64-windows/d3d11.dll.orig");
        assert_eq!(fs::read_to_string(&orig).unwrap(), "wine d3d11");
        assert!(!lib_dir(&app).join("x86_64-unix/winemetal.so.orig").exists());

        // drugi put ne pregazi sacuvani original
        install_files(&rel, &app, "v0.80").unwrap();
        assert_eq!(fs::read_to_string(&orig).unwrap(), "wine d3d11");
        fs::remove_dir_all(&rel).unwrap();
        fs::remove_dir_all(&app).unwrap();
    }
}
