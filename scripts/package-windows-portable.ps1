Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$version = (Get-Content (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json).version
if ($version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') {
    throw 'Invalid application version'
}
$binary = Join-Path $repoRoot 'target/release/shelldock.exe'
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw 'Built application binary is missing (expected target/release/shelldock.exe)'
}

# Verify that the raw app needs no VC++ or WebView2 loader DLL alongside it.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$dumpbin = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' | Select-Object -First 1
if (-not $dumpbin) { throw 'Cannot locate dumpbin to validate portable dependencies' }
$imports = (& $dumpbin /DEPENDENTS $binary | Out-String)
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect application binary dependencies' }
if ($imports -match '(?i)\b(?:VCRUNTIME\d+[^\s]*|MSVCP\d+[^\s]*|WebView2Loader)\.dll\b') {
    throw "Application still requires an unbundled runtime DLL:`n$imports"
}

$scratch = Join-Path ([IO.Path]::GetTempPath()) ('shelldock-portable-' + [guid]::NewGuid())
$stage = Join-Path $scratch 'stage'
$check = Join-Path $scratch 'unpacked'
$output = Join-Path $repoRoot 'target/release/bundle/portable'
$archive = Join-Path $output "ShellDock_${version}_windows_x64_portable.zip"
$appProcess = $null
try {
    New-Item -ItemType Directory -Path $stage, $output -Force | Out-Null
    Get-ChildItem -LiteralPath $output -Filter 'ShellDock_*_windows_x64_portable.zip' -File | Remove-Item -Force
    Copy-Item -LiteralPath $binary -Destination (Join-Path $stage 'ShellDock.exe')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'LICENSE') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $repoRoot 'THIRD_PARTY_NOTICES.md') -Destination $stage
    @'
ShellDock Windows x64 免安装版

完整解压 ZIP 后，双击 ShellDock.exe 启动，不需要安装 ShellDock。
系统需要 Microsoft Edge WebView2 Runtime；此 ZIP 不捆绑该运行时。
缺少时，请从微软官网下载 Evergreen WebView2 Runtime：
https://developer.microsoft.com/microsoft-edge/webview2/

配置仍保存在当前用户的 AppData，密码使用 Windows 凭据管理器，与安装版共享。
此 ZIP 不携带你的连接配置或密码。迁移配置请使用应用内的导入/导出。
项目：https://github.com/super-potato-lab/shelldock
'@ | Set-Content -LiteralPath (Join-Path $stage 'README.txt') -Encoding utf8
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -CompressionLevel Optimal -Force
    Expand-Archive -LiteralPath $archive -DestinationPath $check
    $unpackedBinary = Join-Path $check 'ShellDock.exe'
    if ((Get-FileHash -LiteralPath $binary).Hash -ne (Get-FileHash -LiteralPath $unpackedBinary).Hash) {
        throw 'Portable archive binary differs from the built application'
    }

    # Start the unpacked executable directly, without running the NSIS installer.
    $appProcess = Start-Process -FilePath $unpackedBinary -WorkingDirectory $check -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(25)
    $opened = $false
    while ([DateTime]::UtcNow -lt $deadline) {
        $appProcess.Refresh()
        if ($appProcess.HasExited) { throw "Portable application exited with code $($appProcess.ExitCode)" }
        if ($appProcess.MainWindowHandle -ne 0 -and $appProcess.MainWindowTitle -eq 'ShellDock') {
            Start-Sleep -Seconds 2
            $appProcess.Refresh()
            if ($appProcess.HasExited) { throw 'Portable application exited during startup' }
            $opened = $true
            break
        }
        Start-Sleep -Milliseconds 250
    }
    if (-not $opened) { throw 'Portable application did not open its main window' }
    Write-Output "Portable ZIP verified and application window opened: $archive"
} finally {
    if ($null -ne $appProcess -and -not $appProcess.HasExited) {
        Stop-Process -Id $appProcess.Id -Force -ErrorAction SilentlyContinue
        $appProcess.WaitForExit(5000) | Out-Null
    }
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
