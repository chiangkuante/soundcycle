# Builds the release exe and packages it as MSIX with Microsoft's winapp CLI
# (winget install Microsoft.WinAppCli).
#
#   .\scripts\pack-msix.ps1          Local test package, signed with a dev
#                                    certificate (msix\devcert.pfx).
#   .\scripts\pack-msix.ps1 -Store   Unsigned package for Partner Center;
#                                    the Store signs it after certification.
#
# Output goes to dist-msix\. The version in msix\Package.appxmanifest must be
# raised for every package that should install over a previous one.
param([switch]$Store)

$ErrorActionPreference = 'Stop'
$env:WINAPP_CLI_TELEMETRY_OPTOUT = '1'
$root = Split-Path $PSScriptRoot -Parent
$manifest = Join-Path $root 'msix\Package.appxmanifest'
$layout = Join-Path $root 'src-tauri\target\msix-layout'
$output = Join-Path $root 'dist-msix'

Push-Location $root
try {
    # The frontend is embedded in the exe, so the exe is the whole app.
    npm run tauri build -- --no-bundle
    if ($LASTEXITCODE) { throw 'tauri build failed' }

    Remove-Item $layout -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $layout, $output | Out-Null
    Copy-Item 'src-tauri\target\release\audio-switcher.exe' $layout
    Copy-Item 'msix\Assets', 'msix\Strings' $layout -Recurse

    # resources.pri is built here rather than by winapp pack, whose index only
    # covers images: the manifest's ms-resource: texts live in
    # Strings\<lang>\Resources.resw. The .resw files are compiled in, so they
    # are removed from the layout afterwards.
    $makepri = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makepri.exe" |
        Sort-Object FullName | Select-Object -Last 1
    if (-not $makepri) { throw 'makepri.exe not found (install the Windows SDK)' }
    $priconfig = Join-Path $root 'src-tauri\target\priconfig.xml'
    & $makepri.FullName createconfig /cf $priconfig /dq en-US /pv 10.0.0 /o | Out-Null
    # This is a single package, not a bundle: keep every language and scale
    # in resources.pri instead of splitting them into resource-package PRIs.
    [xml]$config = Get-Content $priconfig -Raw
    [void]$config.resources.RemoveChild($config.resources.packaging)
    $config.Save($priconfig)
    & $makepri.FullName new /pr $layout /cf $priconfig /mn $manifest /of (Join-Path $layout 'resources.pri') /o | Out-Null
    if ($LASTEXITCODE) { throw 'makepri failed' }
    Remove-Item (Join-Path $layout 'Strings') -Recurse

    # Windows PowerShell reads files in the ANSI code page unless told
    # otherwise, which garbles the manifest's non-ASCII text.
    [xml]$xml = Get-Content $manifest -Raw -Encoding UTF8
    $identity = $xml.Package.Identity
    $file = Join-Path $output ("{0}_{1}_x64{2}.msix" -f $identity.Name, $identity.Version, $(if ($Store) { '' } else { '_dev' }))

    if ($Store) {
        winapp pack $layout --manifest $manifest --output $file --skip-pri --no-sign
    } else {
        $cert = Join-Path $root 'msix\devcert.pfx'
        winapp cert generate --manifest $manifest --output $cert --if-exists skip
        winapp pack $layout --manifest $manifest --output $file --skip-pri --cert $cert
    }
    if ($LASTEXITCODE) { throw 'winapp pack failed' }

    Write-Host "Package: $file"
} catch {
    Write-Error $_
    exit 1
} finally {
    Pop-Location
}
