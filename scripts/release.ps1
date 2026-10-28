# Release packaging and publishing script for maxpkg.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/release.ps1 -Version 0.2.0
#   powershell -ExecutionPolicy Bypass -File scripts/release.ps1 -Version 0.2.0 -Publish

param(
    [string]$Version = "",
    [switch]$Publish
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Split-Path -Parent $PSScriptRoot)).Path.Replace('\', '/')
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

Push-Location $root
try {
    # 1. Determine or update version in Cargo.toml and sdk/c/max_hal.h
    $cargoTomlPath = "$root/Cargo.toml"
    $cargoContent = [System.IO.File]::ReadAllText($cargoTomlPath, $utf8NoBom)
    if (-not $Version) {
        if ($cargoContent -match 'version\s*=\s*"(\d+\.\d+\.\d+)"') {
            $Version = $Matches[1]
        } else {
            throw "Could not determine version from Cargo.toml"
        }
    } else {
        $Version = $Version.TrimStart('v')
        if ($Version -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
            throw "Version must be in SemVer format X.Y.Z (got: $Version)"
        }
        $maj = $Matches[1]
        $min = $Matches[2]
        $pat = $Matches[3]

        $cargoContent = [regex]::Replace($cargoContent, '(?m)^version\s*=\s*"\d+\.\d+\.\d+"', "version = `"$Version`"")
        [System.IO.File]::WriteAllText($cargoTomlPath, $cargoContent, $utf8NoBom)

        $cHeaderPath = "$root/sdk/c/max_hal.h"
        $cHeader = [System.IO.File]::ReadAllText($cHeaderPath, $utf8NoBom)
        $cHeader = [regex]::Replace($cHeader, '#define MAX_HAL_VERSION_MAJOR\s+\d+', "#define MAX_HAL_VERSION_MAJOR  $maj")
        $cHeader = [regex]::Replace($cHeader, '#define MAX_HAL_VERSION_MINOR\s+\d+', "#define MAX_HAL_VERSION_MINOR  $min")
        $cHeader = [regex]::Replace($cHeader, '#define MAX_HAL_VERSION_PATCH\s+\d+', "#define MAX_HAL_VERSION_PATCH  $pat")
        $cHeader = [regex]::Replace($cHeader, '#define MAX_HAL_VERSION_STRING\s+"[^"]+"', "#define MAX_HAL_VERSION_STRING `"$Version`"")
        [System.IO.File]::WriteAllText($cHeaderPath, $cHeader, $utf8NoBom)
    }

    $tag = "v$Version"
    Write-Output "=== Preparing Release $tag ==="

    # 2. Cross-compile all binaries and run binary privacy audit
    & "$root/scripts/build_all.ps1"
    if ($LASTEXITCODE -ne 0) { throw "build_all.ps1 failed" }

    # 3. Audit all git-tracked source files for host path leaks
    Write-Output "=== Auditing tracked source files for host path leaks ==="
    $trackedFiles = git ls-files
    $userName = if ($env:USERNAME) { $env:USERNAME } else { $env:USER }
    $srcRegex = "(?i)([a-z]:/users/|[a-z]:\\users\\|" + [regex]::Escape($root) + ")"
    if ($userName -and $userName -notin @("runner", "root") -and $userName.Length -ge 3) {
        $srcRegex = "(?i)([a-z]:/users/|[a-z]:\\users\\|" + [regex]::Escape($root) + "|" + [regex]::Escape($userName) + ")"
    }
    foreach ($f in $trackedFiles) {
        if ($f -match '\.(opus|mp3|wav|png|gif)$') { continue }
        $content = [System.IO.File]::ReadAllText("$root/$f", $utf8NoBom)
        $m = [regex]::Match($content, $srcRegex)
        if ($m.Success) {
            throw "SOURCE PRIVACY AUDIT FAILED: Found '$($m.Value)' in tracked file '$f'"
        }
    }
    Write-Output "[OK] All tracked source files verified clean."

    # 4. Assemble release bundles in dist/v<Version>
    $distDir = "$root/dist/$tag"
    if (Test-Path $distDir) { Remove-Item $distDir -Recurse -Force }
    New-Item -ItemType Directory -Path $distDir | Out-Null

    $relDir = "$root/target/aarch64-linux-android/release"

    Copy-Item "$relDir/maxctl" "$distDir/maxctl-aarch64-linux-android" -Force
    Copy-Item "$relDir/libmax_hal.so" "$distDir/libmax_hal.so" -Force
    Copy-Item "$relDir/libmax_hal.a" "$distDir/libmax_hal.a" -Force

    # 4a. Build maxpkg-runtime-aarch64-v<Version>.tar.gz (ready-to-install Station Max bundle)
    $rtStage = "$distDir/maxpkg-runtime"
    New-Item -ItemType Directory -Path "$rtStage/bin", "$rtStage/lib", "$rtStage/pkgs" -Force | Out-Null
    Copy-Item "$relDir/maxctl" "$rtStage/bin/maxctl" -Force
    Copy-Item "$relDir/libmax_hal.so" "$rtStage/lib/libmax_hal.so" -Force

    foreach ($pkg in @("bt-memes", "pkg-rust", "pkg-c", "pkg-cpp", "pkg-go", "pkg-zig")) {
        $pkgDest = "$rtStage/pkgs/$pkg"
        New-Item -ItemType Directory -Path $pkgDest -Force | Out-Null
        Copy-Item "$root/examples/$pkg/package.toml" "$pkgDest/package.toml" -Force
        Copy-Item "$root/examples/$pkg/$pkg" "$pkgDest/$pkg" -Force
        if (Test-Path "$root/examples/$pkg/sounds") {
            Copy-Item "$root/examples/$pkg/sounds" "$pkgDest/sounds" -Recurse -Force
        }
    }

    $installLines = @(
        "#!/system/bin/sh",
        "set -e",
        'BASE_DIR="/data/local/tmp/max"',
        'STORE_DIR="/data/local/tmp/max-store"',
        'SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"',
        "",
        'mkdir -p "$BASE_DIR/bin" "$BASE_DIR/lib" "$STORE_DIR/pkgs" "$STORE_DIR/run" "$STORE_DIR/logs" "$STORE_DIR/locks" "$STORE_DIR/data"',
        'cp "$SCRIPT_DIR/bin/maxctl" "$BASE_DIR/bin/maxctl"',
        'cp "$SCRIPT_DIR/lib/libmax_hal.so" "$BASE_DIR/lib/libmax_hal.so"',
        'chmod 755 "$BASE_DIR/bin/maxctl"',
        'chmod 644 "$BASE_DIR/lib/libmax_hal.so"',
        "",
        'export LD_LIBRARY_PATH="$BASE_DIR/lib:$LD_LIBRARY_PATH"',
        'for pkg_dir in "$SCRIPT_DIR/pkgs"/*; do',
        '    if [ -d "$pkg_dir" ]; then',
        '        "$BASE_DIR/bin/maxctl" install "$pkg_dir"',
        '    fi',
        'done',
        "",
        'echo ""',
        "echo `"[OK] maxpkg $tag installed to `$BASE_DIR/bin/maxctl`"",
        '"$BASE_DIR/bin/maxctl" list'
    )
    [System.IO.File]::WriteAllText("$rtStage/install-on-station.sh", ($installLines -join "`n") + "`n", $utf8NoBom)

    $rtArchive = "$distDir/maxpkg-runtime-aarch64-$tag.tar.gz"
    tar -czf $rtArchive -C $distDir "maxpkg-runtime"
    Remove-Item $rtStage -Recurse -Force

    # 4b. Build maxpkg-sdk-aarch64-v<Version>.zip (prebuilt libraries + headers for C/C++/Go/Zig/Rust)
    $sdkStage = "$distDir/maxpkg-sdk"
    New-Item -ItemType Directory -Path "$sdkStage/lib", "$sdkStage/include", "$sdkStage/sdk/go/maxhal", "$sdkStage/sdk/zig", "$sdkStage/sdk/rust/src", "$sdkStage/docs" -Force | Out-Null
    Copy-Item "$relDir/libmax_hal.a" "$sdkStage/lib/libmax_hal.a" -Force
    Copy-Item "$relDir/libmax_hal.so" "$sdkStage/lib/libmax_hal.so" -Force
    Copy-Item "$root/sdk/c/max_hal.h" "$sdkStage/include/max_hal.h" -Force
    Copy-Item "$root/sdk/cpp/max_hal.hpp" "$sdkStage/include/max_hal.hpp" -Force
    Copy-Item "$root/sdk/go/go.mod" "$sdkStage/sdk/go/go.mod" -Force
    Copy-Item "$root/sdk/go/maxhal/*.go" "$sdkStage/sdk/go/maxhal/" -Force
    Copy-Item "$root/sdk/zig/maxhal.zig" "$sdkStage/sdk/zig/maxhal.zig" -Force
    Copy-Item "$root/sdk/rust/Cargo.toml" "$sdkStage/sdk/rust/Cargo.toml" -Force
    Copy-Item "$root/sdk/rust/src/lib.rs" "$sdkStage/sdk/rust/src/lib.rs" -Force
    Copy-Item "$root/docs/API.md" "$sdkStage/docs/API.md" -Force
    Copy-Item "$root/README.md" "$sdkStage/README.md" -Force
    Copy-Item "$root/CHANGELOG.md" "$sdkStage/CHANGELOG.md" -Force
    Copy-Item "$root/LICENSE" "$sdkStage/LICENSE" -Force
    Copy-Item "$root/NOTICE" "$sdkStage/NOTICE" -Force

    $sdkArchive = "$distDir/maxpkg-sdk-aarch64-$tag.zip"
    Compress-Archive -Path "$sdkStage/*" -DestinationPath $sdkArchive -Force
    Remove-Item $sdkStage -Recurse -Force

    # 5. Generate SHA256SUMS.txt
    $releaseFiles = @(
        "maxpkg-runtime-aarch64-$tag.tar.gz",
        "maxpkg-sdk-aarch64-$tag.zip",
        "maxctl-aarch64-linux-android",
        "libmax_hal.so",
        "libmax_hal.a"
    )
    $shaLines = @()
    foreach ($rf in $releaseFiles) {
        $hash = (Get-FileHash "$distDir/$rf" -Algorithm SHA256).Hash.ToLower()
        $shaLines += "$hash  $rf"
    }
    [System.IO.File]::WriteAllText("$distDir/SHA256SUMS.txt", ($shaLines -join "`n") + "`n", $utf8NoBom)

    # 6. Extract release notes from CHANGELOG.md
    $changelog = [System.IO.File]::ReadAllText("$root/CHANGELOG.md", $utf8NoBom)
    $verEsc = [regex]::Escape($Version)
    $changelogSection = ""
    if ($changelog -match "(?ms)## \[$verEsc\][^\r\n]*\r?\n(.*?)(?=\r?\n---\r?\n|\r?\n## \[|\z)") {
        $changelogSection = $Matches[1].Trim()
    } else {
        $changelogSection = "Release $tag of maxpkg & libmax_hal."
    }

    $noteLines = @(
        '# maxpkg {TAG} - Native Package Manager, Kernel EventBus & Multi-Language HAL SDK (Pre-Release)',
        '',
        '> [!WARNING]',
        '> **Статус API: Experimental / Unstable (Pre-1.0)**',
        '> Текущая архитектура `libmax_hal`, шина `EventBus` и контракты SDK находятся в активной разработке и ещё не зафиксированы. В следующих версиях возможны полные переработки (rewrites) отдельных модулей и ломающие изменения C-ABI без сохранения обратной совместимости. Всегда используйте `maxctl` и `maxpkg-sdk-aarch64-{TAG}.zip` из одного и того же релиза.',
        '',
        $changelogSection,
        '',
        '---',
        '',
        '## Готовые артефакты релиза (`aarch64-linux-android`, без компиляции)',
        '',
        '| Файл | Назначение |',
        '| :--- | :--- |',
        '| **`maxpkg-runtime-aarch64-{TAG}.tar.gz`** | **Полный комплект для Яндекс Станции Макс**: бинарник `maxctl`, динамическая библиотека `libmax_hal.so`, автоустановщик `install-on-station.sh` и 6 готовых пакетов (`bt-memes`, `pkg-rust`, `pkg-c`, `pkg-cpp`, `pkg-go`, `pkg-zig`) |',
        '| **`maxpkg-sdk-aarch64-{TAG}.zip`** | **SDK для разработчиков пакетов**: предсобранные `libmax_hal.a` и `libmax_hal.so` (`aarch64-linux-android`, API 28), заголовки C99 (`max_hal.h`) и C++17 (`max_hal.hpp`), модули для Go (`maxhal`), Zig (`maxhal.zig`), Rust (`max-sdk`) и `docs/API.md` |',
        '| **`maxctl-aarch64-linux-android`** | Отдельный исполняемый файл пакетного менеджера и супервизора `maxctl` |',
        '| **`libmax_hal.so`** | Отдельная разделяемая библиотека ядра HAL (C-ABI) |',
        '| **`libmax_hal.a`** | Отдельная статическая библиотека ядра HAL для статической линковки |',
        '| **`SHA256SUMS.txt`** | Контрольные суммы SHA-256 всех бинарных файлов релиза |',
        '',
        '---',
        '',
        '## Быстрая установка на Яндекс Станцию Макс',
        '',
        '```bash',
        'tar -xzf maxpkg-runtime-aarch64-{TAG}.tar.gz -C /data/local/tmp/',
        'sh /data/local/tmp/maxpkg-runtime/install-on-station.sh',
        '```',
        '',
        '---',
        '',
        '## Сборка собственного пакета с готовым `maxpkg-sdk-aarch64-{TAG}.zip` (без компиляции Rust)',
        '',
        '```bash',
        '# C99:',
        'aarch64-linux-android28-clang -std=c99 -O2 main.c -I./include -L./lib -lmax_hal -ldl -llog -lm -o my-pkg',
        '',
        '# C++17:',
        'aarch64-linux-android28-clang++ -std=c++17 -O2 -static-libstdc++ main.cpp -I./include -L./lib -lmax_hal -ldl -llog -lm -o my-pkg',
        '```',
        '',
        '---',
        '',
        '## Контрольные суммы (`SHA-256`)',
        '',
        '```text',
        ($shaLines -join "`n"),
        '```'
    ) | ForEach-Object { $_.Replace('{TAG}', $tag) }
    [System.IO.File]::WriteAllText("$distDir/RELEASE_NOTES.md", ($noteLines -join "`n") + "`n", $utf8NoBom)

    Write-Output "[OK] Release artifacts staged in $distDir"

    if ($Publish) {
        if ($env:GITHUB_ACTIONS -eq "true") {
            Write-Output "=== Publishing Release $tag to GitHub (via github-actions[bot]) ==="
            $isPrerelease = $Version.StartsWith("0.")
            $preArgs = if ($isPrerelease) { @("--prerelease") } else { @() }
            $uploadPaths = $releaseFiles | ForEach-Object { "$distDir/$_" }
            $uploadPaths += "$distDir/SHA256SUMS.txt"

            $ErrorActionPreference = "Continue"
            gh release view $tag >$null 2>$null
            $exists = ($LASTEXITCODE -eq 0)
            $ErrorActionPreference = "Stop"

            if ($exists) {
                $editArgs = @("release", "edit", $tag, "--title", "maxpkg $tag (Pre-Release)", "--notes-file", "$distDir/RELEASE_NOTES.md") + $preArgs
                & gh @editArgs
                if ($LASTEXITCODE -ne 0) { throw "gh release edit failed" }
                $upArgs = @("release", "upload", $tag) + $uploadPaths + @("--clobber")
                & gh @upArgs
            } else {
                $createArgs = @("release", "create", $tag) + $uploadPaths + @("--title", "maxpkg $tag (Pre-Release)", "--notes-file", "$distDir/RELEASE_NOTES.md") + $preArgs
                & gh @createArgs
            }
            if ($LASTEXITCODE -ne 0) { throw "gh release failed" }
            Write-Output "[OK] Published GitHub Release $tag successfully!"
        } else {
            Write-Output "=== Pushing tag $tag to trigger anonymous GitHub Actions release (github-actions[bot]) ==="
            git tag -f $tag
            git push -f origin $tag
            if ($LASTEXITCODE -ne 0) { throw "git push tag failed" }
            Write-Output "[OK] Tag $tag pushed. GitHub Actions is now building and publishing the release as github-actions[bot]."
        }
    }
} finally {
    Pop-Location
}
