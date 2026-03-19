$ErrorActionPreference = "Stop"

$ManifestUrl = if ($env:MANIFEST_URL) { $env:MANIFEST_URL } else { "https://downloads.previa.dev/latest.json" }
$PreviaReleaseBaseUrl = if ($env:PREVIA_RELEASE_BASE_URL) { $env:PREVIA_RELEASE_BASE_URL } else { "https://github.com/cloudvibedev/previa/releases/download" }
$PreviaHome = Join-Path $HOME ".previa"
$PreviaBinDir = Join-Path $PreviaHome "bin"
$TargetBinary = Join-Path $PreviaBinDir "previa.exe"

function Write-Info($Message) {
    Write-Host $Message -ForegroundColor Cyan
}

function Write-Success($Message) {
    Write-Host $Message -ForegroundColor Green
}

function Write-WarnMessage($Message) {
    Write-Warning $Message
}

function Fail($Message) {
    Write-Host $Message -ForegroundColor Red
    exit 1
}

function Get-InstallerArchitecture {
    $arch = if ($env:PREVIA_INSTALL_ARCH) { $env:PREVIA_INSTALL_ARCH } else { [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }

    switch ($arch.ToLowerInvariant()) {
        "x64" { return "amd64" }
        "amd64" { return "amd64" }
        "arm64" {
            Write-WarnMessage "Windows arm64 currently installs the published amd64 control binary. Windows x64 emulation may be required."
            return "amd64"
        }
        default { Fail "Unsupported Windows architecture: $arch" }
    }
}

function Invoke-DownloadFile([string]$Url, [string]$Destination) {
    Invoke-WebRequest -Uri $Url -OutFile $Destination
}

function Get-ReleaseAssetUrl([string]$Version, [string]$AssetName) {
    return "$PreviaReleaseBaseUrl/v$Version/$AssetName"
}

function Resolve-BinaryUrl($Manifest, [string]$Version, [string]$ManifestKey, [string]$AssetName) {
    $links = $Manifest.links
    if ($links -and $links.PSObject.Properties.Name -contains $ManifestKey) {
        $value = $links.$ManifestKey
        if ($value) {
            return [string]$value
        }
    }

    Write-WarnMessage "Manifest is missing link '$ManifestKey'. Falling back to GitHub Release asset $AssetName."
    return Get-ReleaseAssetUrl -Version $Version -AssetName $AssetName
}

function Add-UserPathIfMissing([string]$BinDir) {
    $currentUserPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathEntries = @()

    if ($currentUserPath) {
        $pathEntries = $currentUserPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
        foreach ($entry in $pathEntries) {
            if ($entry.TrimEnd('\') -eq $BinDir.TrimEnd('\')) {
                return
            }
        }
    }

    $newUserPath = if ($currentUserPath) { "$BinDir;$currentUserPath" } else { $BinDir }
    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    $env:Path = "$BinDir;$env:Path"
}

Write-Info "Previa installer"
Write-Info "Detecting platform"

$platform = if ($env:PREVIA_INSTALL_OS) { $env:PREVIA_INSTALL_OS } else { [System.Runtime.InteropServices.RuntimeInformation]::OSDescription }
if (-not $platform.ToLowerInvariant().Contains("windows")) {
    Fail "Unsupported operating system: $platform. This installer supports Windows only."
}

$archSlug = Get-InstallerArchitecture
Write-Success "Platform: windows/$archSlug"

$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("previa-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
    $manifestPath = Join-Path $tempDir "latest.json"

    Write-Info "Downloading manifest"
    Invoke-DownloadFile -Url $ManifestUrl -Destination $manifestPath
    $manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json

    $version = [string]$manifest.version
    if (-not $version) {
        Fail "Manifest is invalid: missing version."
    }
    Write-Success "Resolved latest version $version"

    $assetName = "previa-windows-$archSlug.exe"
    $manifestKey = "previa_windows_$archSlug"
    $downloadUrl = Resolve-BinaryUrl -Manifest $manifest -Version $version -ManifestKey $manifestKey -AssetName $assetName

    Write-Info "Installing previa into $PreviaBinDir"
    New-Item -ItemType Directory -Force -Path $PreviaBinDir | Out-Null

    $downloadedBinary = Join-Path $tempDir $assetName
    Invoke-DownloadFile -Url $downloadUrl -Destination $downloadedBinary
    Copy-Item -Force $downloadedBinary $TargetBinary
    Write-Success "Installed previa.exe -> $TargetBinary"

    Write-Info "Configuring PREVIA_HOME and PATH"
    [Environment]::SetEnvironmentVariable("PREVIA_HOME", $PreviaHome, "User")
    $env:PREVIA_HOME = $PreviaHome
    Add-UserPathIfMissing -BinDir $PreviaBinDir

    Write-Success "Previa $version installed successfully."
    Write-Host "Installed directory: $PreviaHome" -ForegroundColor Cyan
    Write-Host "Open a new terminal to use 'previa' from PATH." -ForegroundColor Cyan
}
finally {
    if (Test-Path $tempDir) {
        Remove-Item -Recurse -Force $tempDir
    }
}
