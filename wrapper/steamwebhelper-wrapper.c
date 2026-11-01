/*
 * BridgeWine steamwebhelper wrapper.
 *
 * Steam's CEF helper draws in a separate GPU/renderer process. Wine's macOS
 * driver cannot present that cross-process output (Wine bug 56014), so the
 * Steam window stays black. This wrapper sits in place of steamwebhelper.exe
 * and starts the real helper (steamwebhelper_real.exe, same folder) with
 * extra Chromium switches that keep rendering in one process without GPU.
 *
 * Build: x86_64-w64-mingw32-gcc -O2 -municode -mwindows \
 *          -o steamwebhelper.exe steamwebhelper-wrapper.c
 */
#include <windows.h>
#include <wchar.h>

static const wchar_t REAL_NAME[] = L"steamwebhelper_real.exe";
static const wchar_t EXTRA_ARGS[] = L"--disable-gpu --single-process";

/* Return a pointer past argv[0] in a Windows command line. */
static const wchar_t *skip_argv0(const wchar_t *cmd)
{
    if (*cmd == L'"') {
        cmd++;
        while (*cmd && *cmd != L'"')
            cmd++;
        if (*cmd == L'"')
            cmd++;
    } else {
        while (*cmd && *cmd != L' ' && *cmd != L'\t')
            cmd++;
    }
    while (*cmd == L' ' || *cmd == L'\t')
        cmd++;
    return cmd;
}

int WINAPI wWinMain(HINSTANCE inst, HINSTANCE prev, PWSTR unused, int show)
{
    (void)inst; (void)prev; (void)unused; (void)show;

    wchar_t self[MAX_PATH];
    DWORD n = GetModuleFileNameW(NULL, self, MAX_PATH);
    if (n == 0 || n >= MAX_PATH)
        return 1;

    wchar_t *slash = wcsrchr(self, L'\\');
    size_t dir_len = slash ? (size_t)(slash - self + 1) : 0;
    if (dir_len + wcslen(REAL_NAME) + 1 > MAX_PATH)
        return 1;

    wchar_t real[MAX_PATH];
    wmemcpy(real, self, dir_len);
    wcscpy(real + dir_len, REAL_NAME);

    const wchar_t *rest = skip_argv0(GetCommandLineW());
    /* Our own switches already in front: steamwebhelper_real.exe is a wrapper
     * too, and starting it would recurse forever. */
    if (!wcsncmp(rest, EXTRA_ARGS, wcslen(EXTRA_ARGS)))
        return 2;
    size_t len = wcslen(real) + wcslen(EXTRA_ARGS) + wcslen(rest) + 8;
    wchar_t *cmd = HeapAlloc(GetProcessHeap(), 0, len * sizeof(wchar_t));
    if (!cmd)
        return 1;
    _snwprintf(cmd, len, L"\"%ls\" %ls %ls", real, EXTRA_ARGS, rest);
    cmd[len - 1] = 0;

    STARTUPINFOW si = { .cb = sizeof(si) };
    PROCESS_INFORMATION pi;
    if (!CreateProcessW(real, cmd, NULL, NULL, TRUE, 0, NULL, NULL, &si, &pi))
        return 1;

    WaitForSingleObject(pi.hProcess, INFINITE);
    DWORD code = 1;
    GetExitCodeProcess(pi.hProcess, &code);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    return (int)code;
}
