# Скрипт воспроизводимой и приватной кросс-компиляции ядра `libmax_hal`,
# пакетного менеджера `maxctl` и эталонных пакетов (`Rust`, `C`, `C++`, `Go`, `Zig`)
# под `aarch64-linux-android` (Android API 28).
#
# Скрипт автоматически:
# 1. Загружает локальные пути к тулчейнам из неиндексируемого `.env.local` или переменных окружения.
# 2. Вырезает все пути хост-машины (`--remap-path-prefix`, `-ffile-prefix-map`, `-trimpath`, `llvm-strip`).
# 3. Проводит аудит скомпилированных бинарников и библиотек на отсутствие утечек локальных путей и имени пользователя.

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Split-Path -Parent $PSScriptRoot)).Path.Replace('\', '/')

# 1. Загрузка опционального `.env.local` (не отслеживается в Git)
$envLocal = "$root/.env.local"
if (Test-Path $envLocal) {
    Get-Content $envLocal | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith("#") -and $line.Contains("=")) {
            $parts = $line.Split("=", 2)
            [System.Environment]::SetEnvironmentVariable($parts[0].Trim(), $parts[1].Trim(), "Process")
        }
    }
}

# 2. Обнаружение Android NDK и Zig без хардкода личных путей
$ndkHome = $env:ANDROID_NDK_HOME
if (-not $ndkHome -and $env:ANDROID_NDK_LATEST_HOME -and (Test-Path $env:ANDROID_NDK_LATEST_HOME)) {
    $ndkHome = $env:ANDROID_NDK_LATEST_HOME
}
if (-not $ndkHome -and $env:ANDROID_SDK_ROOT -and (Test-Path "$env:ANDROID_SDK_ROOT/ndk")) {
    $ndkHome = Get-ChildItem "$env:ANDROID_SDK_ROOT/ndk" -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ndkHome -and (Test-Path "/usr/local/lib/android/sdk/ndk")) {
    $ndkHome = Get-ChildItem "/usr/local/lib/android/sdk/ndk" -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ndkHome -and $env:LOCALAPPDATA -and (Test-Path "$env:LOCALAPPDATA/Android/Sdk/ndk")) {
    $ndkHome = Get-ChildItem "$env:LOCALAPPDATA/Android/Sdk/ndk" -Directory | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $ndkHome -or -not (Test-Path $ndkHome)) {
    throw "ANDROID_NDK_HOME is not set or does not exist. Set ANDROID_NDK_HOME or create an untracked .env.local file."
}
$ndkHome = (Resolve-Path $ndkHome).Path.Replace('\', '/')

$hostTag = if ($IsLinux) { "linux-x86_64" } elseif ($IsMacOS) { "darwin-x86_64" } else { "windows-x86_64" }
$exeExt = if ($IsLinux -or $IsMacOS) { "" } else { ".exe" }
$cmdExt = if ($IsLinux -or $IsMacOS) { "" } else { ".cmd" }

$ndkBin = "$ndkHome/toolchains/llvm/prebuilt/$hostTag/bin"
$clang = "$ndkBin/clang$exeExt"
$clangxx = "$ndkBin/clang++$exeExt"
$ndkClangTarget = "$ndkBin/aarch64-linux-android28-clang$cmdExt"

function Find-LlvmTool([string]$name) {
    if (Test-Path "$ndkBin/$name$exeExt") { return "$ndkBin/$name$exeExt" }
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    if ($env:ProgramFiles -and (Test-Path "$env:ProgramFiles/LLVM/bin/$name$exeExt")) {
        return "$($env:ProgramFiles.Replace('\', '/'))/LLVM/bin/$name$exeExt"
    }
    return $null
}

$llvmAr = Find-LlvmTool "llvm-ar"
$llvmStrip = Find-LlvmTool "llvm-strip"

$zig = if ($env:ZIG_HOME) {
    "$($env:ZIG_HOME.Replace('\', '/'))/zig$exeExt"
} elseif (Get-Command zig -ErrorAction SilentlyContinue) {
    (Get-Command zig).Source
} else {
    throw "Zig 0.13+ not found. Set ZIG_HOME or add zig to PATH."
}

# 3. Настройка окружения Cargo и полное скрытие путей хоста (Path Remapping)
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $ndkClangTarget
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_AR = $llvmAr
$env:CC_aarch64_linux_android = $ndkClangTarget
$env:AR_aarch64_linux_android = $llvmAr

$userProfile = $env:USERPROFILE
if (-not $userProfile) { $userProfile = $env:HOME }
$userProfileFwd = $userProfile.Replace('\', '/')
$userProfileBack = $userProfile.Replace('/', '\')
$rootBack = $root.Replace('/', '\')

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { "$userProfile/.cargo" }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { "$userProfile/.rustup" }

$remapFlags = @(
    "--remap-path-prefix=$userProfileFwd=/builder",
    "--remap-path-prefix=$userProfileBack=/builder",
    "--remap-path-prefix=$($rustupHome.Replace('\', '/'))=/rustup",
    "--remap-path-prefix=$($rustupHome.Replace('/', '\'))=/rustup",
    "--remap-path-prefix=$($cargoHome.Replace('\', '/'))=/cargo",
    "--remap-path-prefix=$($cargoHome.Replace('/', '\'))=/cargo",
    "--remap-path-prefix=$root=/workspace",
    "--remap-path-prefix=$rootBack=/workspace"
)
$env:RUSTFLAGS = ($remapFlags -join " ")
$cPrefixFlags = @(
    "-ffile-prefix-map=$root=/workspace",
    "-ffile-prefix-map=$ndkHome=/ndk"
)

$relDir = "$root/target/aarch64-linux-android/release"

Push-Location $root
try {
    Write-Output "=== [1/7] Building Rust workspace (max-hal, max-pkg, maxctl, pkg-rust, bt-memes) ==="
    cargo build --release --target aarch64-linux-android
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    & $llvmStrip --strip-debug "$relDir/libmax_hal.a"
    & $llvmStrip --strip-unneeded "$relDir/maxctl"
    & $llvmStrip --strip-unneeded "$relDir/pkg-rust"
    & $llvmStrip --strip-unneeded "$relDir/bt-memes"

    Copy-Item "$relDir/pkg-rust" "$root/examples/pkg-rust/pkg-rust" -Force
    Copy-Item "$relDir/bt-memes" "$root/examples/bt-memes/bt-memes" -Force

    Write-Output "=== [2/7] Building libmax_hal.so shared C-ABI library ==="
    & $clang "--target=aarch64-linux-android28" @cPrefixFlags "-shared" "-Wl,--whole-archive" "$relDir/libmax_hal.a" "-Wl,--no-whole-archive" "-ldl" "-llog" "-lm" "-lc" "-o" "$relDir/libmax_hal.so"
    if ($LASTEXITCODE -ne 0) { throw "clang shared library build failed" }
    & $llvmStrip --strip-unneeded "$relDir/libmax_hal.so"

    Write-Output "=== [3/7] Building C99 package (examples/pkg-c) ==="
    & $clang "--target=aarch64-linux-android28" @cPrefixFlags "-std=c99" "-O2" "$root/examples/pkg-c/main.c" "-L$relDir" "-lmax_hal" "-ldl" "-llog" "-lm" "-o" "$root/examples/pkg-c/pkg-c"
    if ($LASTEXITCODE -ne 0) { throw "pkg-c build failed" }
    & $llvmStrip --strip-unneeded "$root/examples/pkg-c/pkg-c"

    Write-Output "=== [4/7] Building C++17 RAII package (examples/pkg-cpp) ==="
    & $clangxx "--target=aarch64-linux-android28" @cPrefixFlags "-std=c++17" "-O2" "-static-libstdc++" "$root/examples/pkg-cpp/main.cpp" "-L$relDir" "-lmax_hal" "-ldl" "-llog" "-lm" "-o" "$root/examples/pkg-cpp/pkg-cpp"
    if ($LASTEXITCODE -ne 0) { throw "pkg-cpp build failed" }
    & $llvmStrip --strip-unneeded "$root/examples/pkg-cpp/pkg-cpp"

    Write-Output "=== [5/7] Building Go package (examples/pkg-go) ==="
    Push-Location "$root/examples/pkg-go"
    $env:CGO_ENABLED = "1"
    $env:GOOS = "android"
    $env:GOARCH = "arm64"
    $env:CC = $ndkClangTarget
    $env:CGO_CFLAGS = ($cPrefixFlags -join " ")
    $env:CGO_LDFLAGS = "-L$relDir -lmax_hal -ldl -llog -lm"
    go build -trimpath -ldflags="-s -w" -o pkg-go .
    if ($LASTEXITCODE -ne 0) { throw "pkg-go build failed" }
    Pop-Location
    & $llvmStrip --strip-unneeded "$root/examples/pkg-go/pkg-go"

    Write-Output "=== [6/7] Building Zig package (examples/pkg-zig) ==="
    Copy-Item "$root/sdk/zig/maxhal.zig" "$root/examples/pkg-zig/maxhal.zig" -Force
    & $zig build-obj "$root/examples/pkg-zig/main.zig" -target aarch64-linux-android -O ReleaseSmall -fstrip -fPIC "-femit-bin=$root/examples/pkg-zig/pkg_zig.o"
    if ($LASTEXITCODE -ne 0) { throw "zig build-obj failed" }
    & $clang "--target=aarch64-linux-android28" @cPrefixFlags "-O2" "$root/examples/pkg-zig/pkg_zig.o" "-L$relDir" "-lmax_hal" "-ldl" "-llog" "-lm" "-lc" "-o" "$root/examples/pkg-zig/pkg-zig"
    if ($LASTEXITCODE -ne 0) { throw "pkg-zig link failed" }
    Remove-Item "$root/examples/pkg-zig/pkg_zig.o*", "$root/examples/pkg-zig/maxhal.zig" -Force -ErrorAction SilentlyContinue
    & $llvmStrip --strip-unneeded "$root/examples/pkg-zig/pkg-zig"

    Write-Output "=== [7/7] Privacy Audit: scanning binaries for leaked host paths ==="
    $artifacts = @(
        "$relDir/libmax_hal.a",
        "$relDir/libmax_hal.so",
        "$relDir/maxctl",
        "$root/examples/pkg-rust/pkg-rust",
        "$root/examples/bt-memes/bt-memes",
        "$root/examples/pkg-c/pkg-c",
        "$root/examples/pkg-cpp/pkg-cpp",
        "$root/examples/pkg-go/pkg-go",
        "$root/examples/pkg-zig/pkg-zig"
    )
    $userName = $env:USERNAME
    if (-not $userName) { $userName = $env:USER }
    $forbiddenPatterns = @(
        "[A-Za-z]:\\Users\\",
        "[A-Za-z]:/Users/",
        [regex]::Escape($root),
        [regex]::Escape($rootBack)
    )
    if ($userName -and $userName -notin @("runner", "root") -and $userName.Length -ge 3) {
        $forbiddenPatterns += [regex]::Escape($userName)
    }
    $combinedRegex = "(" + ($forbiddenPatterns -join "|") + ")"

    foreach ($art in $artifacts) {
        $text = [System.IO.File]::ReadAllText($art, [System.Text.Encoding]::ASCII)
        $m = [regex]::Match($text, $combinedRegex, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase)
        if ($m.Success) {
            throw "PRIVACY AUDIT FAILED: Found leaked host string '$($m.Value)' inside $art"
        }
    }

    Write-Output "`n[OK] All binaries built, stripped, and verified clean (0 host paths leaked)!"
} finally {
    Pop-Location
}
