//! Nasi Wine moduli: zvanicni Wine izvorni kod + patch-evi iz `patches/`.
//!
//! WineHQ 11.21 na macOS-u ne radi sa igrama cija zastita zove direktne syscall-ove
//! i otvara svoj exe preko `\\.\GLOBALROOT\??\C:\...` (AoE3 DE), a pod Rosettom
//! prijavljuje CPU od 1000 MHz. Ispravke su u `patches/wine-<ver>/`; ovde ih
//! primenjujemo na izvorni kod sa dl.winehq.org, gradimo samo izmenjene module
//! (x86_64, preko Rosette) i menjamo ih u runtime-u. Originali ostaju pored kao `*.orig`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::bottle::{download_invocations, home, run_all, Invocation};
use crate::runtime::{runtimes_dir, WINEHQ_VERSION};

pub const PATCHES: [(&str, &str); 4] = [
    (
        "0001-ntdll-restore-TEB-GSBASE-after-SIGSYS-on-macOS.patch",
        include_str!(
            "../patches/wine-11.21/0001-ntdll-restore-TEB-GSBASE-after-SIGSYS-on-macOS.patch"
        ),
    ),
    (
        "0002-ntdll-resolve-GLOBALROOT-and-GLOBAL-prefixes.patch",
        include_str!(
            "../patches/wine-11.21/0002-ntdll-resolve-GLOBALROOT-and-GLOBAL-prefixes.patch"
        ),
    ),
    (
        "0003-wineboot-dont-report-cpu-clock-below-max-mhz.patch",
        include_str!(
            "../patches/wine-11.21/0003-wineboot-dont-report-cpu-clock-below-max-mhz.patch"
        ),
    ),
    (
        "0004-kernelbase-start-steamwebhelper-with-software-rendering.patch",
        include_str!(
            "../patches/wine-11.21/0004-kernelbase-start-steamwebhelper-with-software-rendering.patch"
        ),
    ),
];

/// Moduli koje patch-evi menjaju: putanja u build folderu -> putanja u `lib/wine/`.
pub const TARGETS: [(&str, &str); 3] = [
    ("dlls/ntdll/ntdll.so", "x86_64-unix/ntdll.so"),
    (
        "dlls/kernelbase/x86_64-windows/kernelbase.dll",
        "x86_64-windows/kernelbase.dll",
    ),
    (
        "programs/wineboot/x86_64-windows/wineboot.exe",
        "x86_64-windows/wineboot.exe",
    ),
];

/// Bez ovoga configure trazi biblioteke koje nasi moduli ne koriste.
const CONFIGURE_ARGS: &[&str] = &[
    "--enable-archs=x86_64",
    "--without-x",
    "--without-freetype",
    "--without-gnutls",
    "--without-gstreamer",
    "--without-sdl",
    "--without-cups",
    "--without-sane",
    "--without-gphoto",
    "--without-krb5",
    "--without-netapi",
    "--without-pcap",
    "--without-usb",
    "--without-v4l2",
    "--without-vulkan",
    "--without-opencl",
    "--without-ffmpeg",
    "--without-capi",
    "--without-inotify",
    "--without-pulse",
    "--without-alsa",
    "--without-oss",
    "--without-dbus",
    "--without-udev",
];

pub fn source_url(version: &str) -> String {
    let major = version.split('.').next().unwrap_or(version);
    format!("https://dl.winehq.org/wine/source/{major}.x/wine-{version}.tar.xz")
}

/// Gotovi moduli iz CI-ja (.github/workflows/wine-patched.yml) su na GitHub release-u.
pub const RELEASE_REPO: &str = "NNikolaG/bridgewine";

/// FNV-1a preko imena i sadrzaja patch-eva: svaka izmena u `patches/` daje novi tag,
/// pa `bw` uvek trazi module bas za patch-eve koje nosi u sebi.
pub fn patch_set_id() -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (name, text) in PATCHES {
        for b in name.bytes().chain([0]).chain(text.bytes()).chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{:012x}", h >> 16)
}

pub fn release_tag(version: &str) -> String {
    format!("wine-{version}-bw-{}", patch_set_id())
}

pub fn asset_name(version: &str) -> String {
    format!("bw-wine-{version}-patched.tar.gz")
}

pub fn prebuilt_url(version: &str) -> String {
    format!(
        "https://github.com/{RELEASE_REPO}/releases/download/{}/{}",
        release_tag(version),
        asset_name(version)
    )
}

/// `<app>/Contents/Resources/wine/bin/wine` -> `<app>`
pub fn app_for(wine: &str) -> Option<PathBuf> {
    Path::new(wine).ancestors().nth(5).map(Path::to_path_buf)
}

fn lib_dir(app: &Path) -> PathBuf {
    app.join("Contents/Resources/wine/lib/wine")
}

/// Zapis u `lib/wine/`: patch-evi i velicine nasih modula, da `bw run` primeti
/// kad se runtime ponovo instalira.
fn stamp_path(app: &Path) -> PathBuf {
    lib_dir(app).join("bw-patched.txt")
}

fn stamp_text(sizes: &[u64]) -> String {
    let mut s = String::new();
    for ((_, lib), size) in TARGETS.iter().zip(sizes) {
        s.push_str(&format!("{lib} {size}\n"));
    }
    for (name, _) in PATCHES {
        s.push_str(name);
        s.push('\n');
    }
    s
}

fn installed_sizes(app: &Path) -> Option<Vec<u64>> {
    TARGETS
        .iter()
        .map(|(_, lib)| fs::metadata(lib_dir(app).join(lib)).ok().map(|m| m.len()))
        .collect()
}

/// Da li runtime koristi nase module sa svim trenutnim patch-evima.
pub fn is_installed(app: &Path) -> bool {
    let Some(sizes) = installed_sizes(app) else {
        return false;
    };
    fs::read_to_string(stamp_path(app)).is_ok_and(|s| s == stamp_text(&sizes))
}

fn sh(dir: &Path, cmd: &[&str], env: &[(&str, String)]) -> Invocation {
    let mut args = vec![
        "-c".to_string(),
        r#"cd "$1" && shift && exec "$@""#.to_string(),
        "sh".to_string(),
        dir.display().to_string(),
    ];
    args.extend(cmd.iter().map(|s| s.to_string()));
    Invocation {
        program: "sh".into(),
        args,
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

/// Raspakuj -> patch-evi -> configure -> make samo nasih modula. `src` = `$BW_HOME/src`.
pub fn build_invocations(
    src: &Path,
    tarball: &Path,
    version: &str,
    path_env: &str,
) -> Vec<Invocation> {
    let tree = src.join(format!("wine-{version}"));
    let build = src.join(format!("build-{version}"));
    let patches = src.join(format!("patches-{version}"));
    let env = [("PATH", path_env.to_string())];
    let mut v = vec![
        sh(
            src,
            &[
                "rm",
                "-rf",
                &tree.display().to_string(),
                &build.display().to_string(),
            ],
            &[],
        ),
        sh(src, &["tar", "-xJf", &tarball.display().to_string()], &[]),
    ];
    for (name, _) in PATCHES {
        let p = patches.join(name).display().to_string();
        v.push(sh(&tree, &["patch", "-p1", "--forward", "-i", &p], &[]));
    }
    v.push(sh(src, &["mkdir", "-p", &build.display().to_string()], &[]));
    let configure = format!("../wine-{version}/configure");
    let mut cfg = vec![
        "arch",
        "-x86_64",
        configure.as_str(),
        "CC=clang -arch x86_64",
    ];
    cfg.extend_from_slice(CONFIGURE_ARGS);
    v.push(sh(&build, &cfg, &env));
    let jobs = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .to_string();
    let mut make = vec!["arch", "-x86_64", "make", "-j", &jobs];
    make.extend(TARGETS.iter().map(|(b, _)| *b));
    v.push(sh(&build, &make, &env));
    v
}

fn tool(program: &str, args: &[&str], file: &Path) -> Invocation {
    let mut a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    a.push(file.display().to_string());
    Invocation {
        program: program.into(),
        args: a,
        env: Default::default(),
    }
}

fn err(p: &Path) -> impl Fn(std::io::Error) -> String + '_ {
    move |e| format!("{}: {e}", p.display())
}

/// codesign za .so (Gatekeeper), strip za PE samo kad imamo mingw (lokalni build).
fn fixup(file: &Path, lib: &str, strip: bool) -> Result<(), String> {
    let inv = if lib.ends_with(".so") {
        tool("codesign", &["-f", "-s", "-"], file)
    } else if strip {
        tool("x86_64-w64-mingw32-strip", &[], file)
    } else {
        return Ok(());
    };
    run_all(&[inv], false)
}

/// Ubaci module u runtime. `src(built, lib)` kaze gde je nas fajl.
fn install_from(
    src: impl Fn(&str, &str) -> PathBuf,
    app: &Path,
    strip: bool,
) -> Result<(), String> {
    for (built, lib) in TARGETS {
        let target = lib_dir(app).join(lib);
        let orig = target.with_file_name(format!("{}.orig", lib.rsplit('/').next().unwrap_or(lib)));
        if !orig.exists() {
            fs::copy(&target, &orig).map_err(err(&orig))?;
        }
        // preko privremenog fajla: procesi koji vec rade zadrze stari modul
        let tmp = target.with_extension("bw-new");
        let from = src(built, lib);
        fs::copy(&from, &tmp).map_err(err(&from))?;
        fixup(&tmp, lib, strip)?;
        fs::rename(&tmp, &target).map_err(err(&target))?;
    }
    let sizes = installed_sizes(app).ok_or("nedostaje instalirani modul")?;
    fs::write(stamp_path(app), stamp_text(&sizes)).map_err(err(&stamp_path(app)))
}

/// Folder za release: `<lib>` putanje kao u `lib/wine/` + `bw-release.txt`.
fn export(build: &Path, out: &Path, version: &str) -> Result<(), String> {
    for (built, lib) in TARGETS {
        let dst = out.join(lib);
        fs::create_dir_all(dst.parent().unwrap()).map_err(err(&dst))?;
        fs::copy(build.join(built), &dst).map_err(err(&dst))?;
        fixup(&dst, lib, true)?;
    }
    let mut info = format!("wine {version}\ntag {}\n", release_tag(version));
    for (name, _) in PATCHES {
        info.push_str(&format!("patch {name}\n"));
    }
    let p = out.join("bw-release.txt");
    fs::write(&p, info).map_err(err(&p))
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

fn runtime_app(home: &Path, version: &str) -> PathBuf {
    runtimes_dir(home).join(format!("wine-devel-{version}.app"))
}

/// Gradi module; sa `out` ih samo spakuje u folder (CI), bez runtime-a.
pub fn cmd_build(dry_run: bool, out: Option<PathBuf>) -> ExitCode {
    finish(build(dry_run, out))
}

pub fn build(dry_run: bool, out: Option<PathBuf>) -> Result<(), String> {
    let home = home();
    let version = WINEHQ_VERSION;
    let app = runtime_app(&home, version);
    if !dry_run && out.is_none() && !app.exists() {
        return Err("WineHQ nije instaliran; prvo `bw runtime install`".to_string());
    }
    let src = home.join("src");
    let tarball = home.join("cache").join(format!("wine-{version}.tar.xz"));
    let patches = src.join(format!("patches-{version}"));
    if !dry_run {
        fs::create_dir_all(&patches).map_err(|e| e.to_string())?;
        fs::create_dir_all(home.join("cache")).map_err(|e| e.to_string())?;
        for (name, text) in PATCHES {
            fs::write(patches.join(name), text).map_err(|e| e.to_string())?;
        }
    }
    if !tarball.exists() {
        run_all(
            &download_invocations(&source_url(version), &tarball),
            dry_run,
        )?;
    }
    let path = format!(
        "/opt/homebrew/opt/bison/bin:/opt/homebrew/opt/flex/bin:{}",
        std::env::var("PATH").unwrap_or_default()
    );
    run_all(&build_invocations(&src, &tarball, version, &path), dry_run)?;
    if dry_run {
        return Ok(());
    }
    let build = src.join(format!("build-{version}"));
    if let Some(out) = out {
        export(&build, &out, version)?;
        println!(
            "moduli za release {} u {}",
            release_tag(version),
            out.display()
        );
        return Ok(());
    }
    install_from(|b, _| build.join(b), &app, true)?;
    println!(
        "Wine moduli sa BridgeWine patch-evima instalirani u {}",
        app.display()
    );
    Ok(())
}

/// curl -> raspakuj u privremeni folder (bez kompajliranja na Mac-u).
pub fn fetch_invocations(home: &Path, version: &str) -> (Vec<Invocation>, PathBuf) {
    let cache = home.join("cache");
    let tarball = cache.join(format!("{}-{}", patch_set_id(), asset_name(version)));
    let staging = cache.join(format!("bw-wine-{version}-{}", patch_set_id()));
    let (t, st) = (tarball.display().to_string(), staging.display().to_string());
    let cmd = |program: &str, args: &[&str]| Invocation {
        program: program.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        env: Default::default(),
    };
    let mut v = Vec::new();
    if !tarball.exists() {
        v.push(cmd("curl", &["-fL", "-o", &t, &prebuilt_url(version)]));
    }
    v.push(cmd("rm", &["-rf", &st]));
    v.push(cmd("mkdir", &["-p", &st]));
    v.push(cmd("tar", &["-xzf", &t, "-C", &st]));
    (v, staging)
}

/// Skini gotove module sa GitHub release-a i ubaci ih u runtime.
pub fn cmd_install_prebuilt(dry_run: bool) -> ExitCode {
    finish(install_prebuilt(dry_run))
}

pub fn install_prebuilt(dry_run: bool) -> Result<(), String> {
    let home = home();
    let version = WINEHQ_VERSION;
    let app = runtime_app(&home, version);
    if !dry_run && !app.exists() {
        return Err("WineHQ nije instaliran; prvo `bw runtime install`".to_string());
    }
    if !dry_run && is_installed(&app) {
        println!(
            "BridgeWine Wine moduli ({}) vec instalirani",
            release_tag(version)
        );
        return Ok(());
    }
    if !dry_run {
        fs::create_dir_all(home.join("cache")).map_err(|e| e.to_string())?;
    }
    let (steps, staging) = fetch_invocations(&home, version);
    run_all(&steps, dry_run).map_err(|e| {
        format!(
            "{e}\nnema gotovih modula za {}; napravi ih sam: `bw runtime build-patched`",
            release_tag(version)
        )
    })?;
    if dry_run {
        return Ok(());
    }
    install_from(|_, lib| staging.join(lib), &app, false)?;
    let _ = fs::remove_dir_all(&staging);
    println!(
        "Wine moduli {} instalirani u {}",
        release_tag(version),
        app.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_is_five_levels_above_wine() {
        assert_eq!(
            app_for("/r/w.app/Contents/Resources/wine/bin/wine").unwrap(),
            PathBuf::from("/r/w.app")
        );
    }

    #[test]
    fn source_comes_from_winehq() {
        assert_eq!(
            source_url("11.21"),
            "https://dl.winehq.org/wine/source/11.x/wine-11.21.tar.xz"
        );
    }

    #[test]
    fn patches_only_touch_modules_we_build() {
        for (name, text) in PATCHES {
            assert!(text.starts_with("Subject: [PATCH] "), "{name}");
            for line in text.lines().filter(|l| l.starts_with("diff --git a/")) {
                let file = line.split(' ').nth(2).unwrap().trim_start_matches("a/");
                let dir = &file[..file.rfind('/').unwrap()];
                let dir = dir.trim_end_matches("/unix");
                assert!(
                    TARGETS.iter().any(|(b, _)| b.starts_with(dir)),
                    "{name}: {file}"
                );
            }
        }
    }

    #[test]
    fn build_patches_then_makes_only_our_modules() {
        let inv = build_invocations(Path::new("/s"), Path::new("/c/w.tar.xz"), "11.21", "/p");
        let cmds: Vec<String> = inv.iter().map(|i| i.args[4..].join(" ")).collect();
        assert_eq!(cmds[0], "rm -rf /s/wine-11.21 /s/build-11.21");
        assert_eq!(cmds[1], "tar -xJf /c/w.tar.xz");
        for (i, (name, _)) in PATCHES.iter().enumerate() {
            assert_eq!(
                cmds[2 + i],
                format!("patch -p1 --forward -i /s/patches-11.21/{name}")
            );
        }
        assert_eq!(inv[2].args[3], "/s/wine-11.21");
        let n = 2 + PATCHES.len();
        assert!(
            cmds[n + 1].starts_with("arch -x86_64 ../wine-11.21/configure CC=clang -arch x86_64")
        );
        assert!(cmds[n + 2].ends_with(
            "dlls/ntdll/ntdll.so dlls/kernelbase/x86_64-windows/kernelbase.dll \
             programs/wineboot/x86_64-windows/wineboot.exe"
        ));
        assert_eq!(inv[n + 2].args[3], "/s/build-11.21");
        assert_eq!(inv[n + 2].env["PATH"], "/p");
    }

    #[test]
    fn release_tag_follows_patch_set() {
        let tag = release_tag("11.21");
        assert!(tag.starts_with("wine-11.21-bw-"), "{tag}");
        assert_eq!(patch_set_id().len(), 12);
        assert_eq!(tag, release_tag("11.21"));
        assert_eq!(
            prebuilt_url("11.21"),
            format!(
                "https://github.com/NNikolaG/bridgewine/releases/download/{tag}/bw-wine-11.21-patched.tar.gz"
            )
        );
    }

    #[test]
    fn prebuilt_downloads_then_unpacks_into_staging() {
        let home = Path::new("/nonexistent-bw-home");
        let (inv, staging) = fetch_invocations(home, "11.21");
        let progs: Vec<&str> = inv.iter().map(|i| i.program.as_str()).collect();
        assert_eq!(progs, ["curl", "rm", "mkdir", "tar"]);
        assert!(inv[0].args.contains(&prebuilt_url("11.21")));
        assert!(staging.starts_with("/nonexistent-bw-home/cache"));
        assert_eq!(inv[3].args.last().unwrap(), &staging.display().to_string());
    }

    #[test]
    fn stamp_tracks_sizes_and_patches() {
        let app = std::env::temp_dir().join(format!("bw-wine-{}", std::process::id()));
        for (_, lib) in TARGETS {
            let p = lib_dir(&app).join(lib);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, b"ours").unwrap();
        }
        assert!(!is_installed(&app));
        fs::write(stamp_path(&app), stamp_text(&[4; TARGETS.len()])).unwrap();
        assert!(is_installed(&app));
        // runtime ponovo instaliran: drugi ntdll.so
        fs::write(lib_dir(&app).join(TARGETS[0].1), b"gcenx build").unwrap();
        assert!(!is_installed(&app));
        fs::remove_dir_all(&app).unwrap();
    }
}
