//! Binarna zakrpa za `ntdll.so` iz WineHQ 11.21 (Gcenx build).
//!
//! `sigsys_handler` na macOS-u preko `init_handler` prebaci GSBASE na pthread TSD,
//! ali ga, za razliku od ostalih handlera (`leave_handler`), ne vrati na TEB pre
//! skoka u `__wine_syscall_dispatcher`. Dispatcher onda cita `%gs:0x30` kao TEB,
//! dobije NULL i pukne na `0x320(%r13)`. Direktan `syscall` iz PE koda (anti-tamper
//! u AoE3 DE / AoE2 DE zove tako `NtAllocateVirtualMemory`) zato vraca
//! STATUS_ACCESS_VIOLATION i igra pada pri startu.
//!
//! Zakrpa: kraj handlera (`RIP_sig = prolog_end; return`) skace u TRACE blok istog
//! handlera (radi samo uz `WINEDEBUG=+seh`, pa ga gasimo), koji sada uradi
//! `RIP_sig = prolog_end`, pa repni poziv `_thread_set_tsd_base(teb)`; TEB je prvi
//! pokazivac na dnu signal steka (`*(rsp & ~0xffff)`), isto kao u `init_handler`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Hunk {
    off: usize,
    old: &'static [u8],
    new: &'static [u8],
}

const INT3: u8 = 0xcc;

#[rustfmt::skip]
const HUNKS: [Hunk; 4] = [
    // jne trace -> nop nop
    Hunk { off: 0x38c9f, old: &[0x75, 0x0e], new: &[0x90, 0x90] },
    // je skip_trace -> jmp skip_trace
    Hunk { off: 0x38cad, old: &[0x74, 0x30], new: &[0xeb, 0x30] },
    // TRACE blok -> novi kraj handlera
    Hunk {
        off: 0x38caf,
        old: &[
            0x48, 0x8b, 0x43, 0x30, 0x4c, 0x8b, 0x40, 0x10, 0x4c, 0x8b, 0x88, 0x90, 0x00, 0x00, 0x00, 0x48,
            0x8d, 0x35, 0x27, 0x1f, 0x04, 0x00, 0x48, 0x8d, 0x15, 0x21, 0x40, 0x03, 0x00, 0x48, 0x8d, 0x0d,
            0x29, 0x40, 0x03, 0x00, 0xbf, 0x03, 0x00, 0x00, 0x00, 0x31, 0xc0, 0xe8, 0x01, 0x65, 0xff, 0xff,
        ],
        new: &[
            0x48, 0x8b, 0x0d, 0xd2, 0x1e, 0x04, 0x00,       // mov prolog_end_ptr(%rip), %rcx
            0x48, 0x89, 0x88, 0x90, 0x00, 0x00, 0x00,       // mov %rcx, 0x90(%rax)   ; RIP_sig
            0x48, 0x89, 0xe7,                               // mov %rsp, %rdi
            0x66, 0x31, 0xff,                               // xor %di, %di
            0x48, 0x8b, 0x3f,                               // mov (%rdi), %rdi       ; teb
            0x5b, 0x41, 0x5e, 0x5d,                         // pop %rbx; pop %r14; pop %rbp
            0xe9, 0x09, 0xdc, 0x02, 0x00,                   // jmp _thread_set_tsd_base
            INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3,
        ],
    },
    // stari kraj handlera -> jmp na novi
    Hunk {
        off: 0x38d71,
        old: &[
            0x48, 0x8d, 0x0d, 0x10, 0x1e, 0x04, 0x00, 0x48, 0x8b, 0x09, 0x48, 0x89, 0x88, 0x90, 0x00, 0x00,
            0x00, 0x5b, 0x41, 0x5e, 0x5d, 0xc3,
        ],
        new: &[
            0xe9, 0x39, 0xff, 0xff, 0xff,
            INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3, INT3,
        ],
    },
];

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Patched,
    AlreadyPatched,
    /// Drugi build ntdll.so; ne diramo ga.
    UnknownBuild,
}

fn matches(buf: &[u8], pick: fn(&Hunk) -> &'static [u8]) -> bool {
    HUNKS.iter().all(|h| {
        let b = pick(h);
        buf.get(h.off..h.off + b.len()) == Some(b)
    })
}

pub fn patch_bytes(buf: &mut [u8]) -> State {
    if matches(buf, |h| h.new) {
        return State::AlreadyPatched;
    }
    if !matches(buf, |h| h.old) {
        return State::UnknownBuild;
    }
    for h in &HUNKS {
        buf[h.off..h.off + h.new.len()].copy_from_slice(h.new);
    }
    State::Patched
}

/// `<app>/Contents/Resources/wine/bin/wine` -> `.../wine/lib/wine/x86_64-unix/ntdll.so`
pub fn ntdll_so_for(wine: &str) -> Option<PathBuf> {
    let root = Path::new(wine).parent()?.parent()?;
    Some(root.join("lib/wine/x86_64-unix/ntdll.so"))
}

/// Zakrpi ntdll.so na disku (preko privremenog fajla, da procesi koji ga vec
/// koriste zadrze stari) i ponovo ga ad-hoc potpisi.
pub fn apply(path: &Path) -> Result<State, String> {
    let err = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut buf = fs::read(path).map_err(err)?;
    let state = patch_bytes(&mut buf);
    if state != State::Patched {
        return Ok(state);
    }
    let tmp = path.with_extension("so.bw-new");
    fs::write(&tmp, &buf).map_err(err)?;
    let perms = fs::metadata(path).map_err(err)?.permissions();
    fs::set_permissions(&tmp, perms).map_err(err)?;
    let signed = Command::new("codesign")
        .args(["-f", "-s", "-"])
        .arg(&tmp)
        .status()
        .map_err(|e| format!("codesign: {e}"))?
        .success();
    if !signed {
        let _ = fs::remove_file(&tmp);
        return Err(format!("codesign nije uspeo za {}", tmp.display()));
    }
    fs::rename(&tmp, path).map_err(err)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn original() -> Vec<u8> {
        let mut buf = vec![0u8; 0x39000];
        for h in &HUNKS {
            buf[h.off..h.off + h.old.len()].copy_from_slice(h.old);
        }
        buf
    }

    #[test]
    fn hunks_keep_their_size() {
        for h in &HUNKS {
            assert_eq!(h.old.len(), h.new.len(), "{:#x}", h.off);
        }
    }

    #[test]
    fn patches_once_then_recognizes_patched() {
        let mut buf = original();
        assert_eq!(patch_bytes(&mut buf), State::Patched);
        assert_eq!(&buf[0x38d71..0x38d76], [0xe9, 0x39, 0xff, 0xff, 0xff]);
        let snapshot = buf.clone();
        assert_eq!(patch_bytes(&mut buf), State::AlreadyPatched);
        assert_eq!(buf, snapshot);
    }

    #[test]
    fn leaves_unknown_builds_alone() {
        let mut buf = original();
        buf[0x38c9f] = 0x00;
        let snapshot = buf.clone();
        assert_eq!(patch_bytes(&mut buf), State::UnknownBuild);
        assert_eq!(buf, snapshot);
        assert_eq!(patch_bytes(&mut [0u8; 16]), State::UnknownBuild);
    }

    #[test]
    fn jumps_land_where_intended() {
        // rel32 se racuna od kraja instrukcije
        let rel = |off: usize, len: usize, bytes: &[u8]| {
            let d = i32::from_le_bytes(bytes[len - 4..len].try_into().unwrap());
            (off + len) as i64 + d as i64
        };
        let cave = &HUNKS[2];
        assert_eq!(rel(cave.off, 7, cave.new), 0x7ab88); // prolog_end_ptr, kao original
        assert_eq!(rel(cave.off + 27, 5, &cave.new[27..]), 0x668d8); // tsd_base stub
        assert_eq!(rel(HUNKS[3].off, 5, HUNKS[3].new), cave.off as i64);
    }

    #[test]
    fn finds_ntdll_next_to_wine() {
        assert_eq!(
            ntdll_so_for("/r/w.app/Contents/Resources/wine/bin/wine").unwrap(),
            PathBuf::from("/r/w.app/Contents/Resources/wine/lib/wine/x86_64-unix/ntdll.so")
        );
    }
}
