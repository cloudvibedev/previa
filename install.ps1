$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$ManifestUrl = 'https://downloads.previa.dev/manifest.json'

function Write-Step {
    param([string] $Message)
    Write-Host $Message -ForegroundColor Blue
}

function Write-Success {
    param([string] $Message)
    Write-Host $Message -ForegroundColor Green
}

function Write-WarningLine {
    param([string] $Message)
    Write-Host $Message -ForegroundColor Yellow
}

function Fail {
    param([string] $Message)
    Write-Host $Message -ForegroundColor Red
    exit 1
}

function Get-ManifestLink {
    param(
        [Parameter(Mandatory = $true)][pscustomobject] $Manifest,
        [Parameter(Mandatory = $true)][string] $Key
    )

    $property = $Manifest.links.PSObject.Properties[$Key]
    if (-not $property -or [string]::IsNullOrWhiteSpace([string] $property.Value)) {
        Fail "Manifest is missing link '$Key'."
    }

    return [string] $property.Value
}

function Download-FileWithProgress {
    param(
        [Parameter(Mandatory = $true)][string] $Url,
        [Parameter(Mandatory = $true)][string] $Destination,
        [Parameter(Mandatory = $true)][string] $Activity
    )

    $httpClient = [System.Net.Http.HttpClient]::new()
    try {
        $response = $httpClient.GetAsync($Url, [System.Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
        $response.EnsureSuccessStatusCode()

        $totalBytes = $response.Content.Headers.ContentLength
        $downloadedBytes = 0L
        $buffer = New-Object byte[] 65536
        $stream = $response.Content.ReadAsStream()
        $fileStream = [System.IO.File]::Open($Destination, [System.IO.FileMode]::Create, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)

        try {
            while (($read = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                $fileStream.Write($buffer, 0, $read)
                $downloadedBytes += $read

                if ($totalBytes) {
                    $percentComplete = [int](($downloadedBytes * 100) / $totalBytes)
                    Write-Progress -Activity $Activity -Status "$percentComplete% ($downloadedBytes / $totalBytes bytes)" -PercentComplete $percentComplete
                }
                else {
                    Write-Progress -Activity $Activity -Status "$downloadedBytes bytes downloaded" -PercentComplete 0
                }
            }
        }
        finally {
            $fileStream.Dispose()
            $stream.Dispose()
            Write-Progress -Activity $Activity -Completed
        }
    }
    finally {
        $httpClient.Dispose()
    }
}

function Add-PathEntry {
    param([Parameter(Mandatory = $true)][string] $PathValue)

    $currentUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @()
    if (-not [string]::IsNullOrWhiteSpace($currentUserPath)) {
        $entries = $currentUserPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
    }

    $normalizedTarget = $PathValue.TrimEnd('\')
    $alreadyPresent = $false
    foreach ($entry in $entries) {
        if ($entry.TrimEnd('\').Equals($normalizedTarget, [System.StringComparison]::OrdinalIgnoreCase)) {
            $alreadyPresent = $true
            break
        }
    }

    if (-not $alreadyPresent) {
        $updatedEntries = @($entries + $PathValue)
        [Environment]::SetEnvironmentVariable('Path', ($updatedEntries -join ';'), 'User')
    }

    $processEntries = $env:Path.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
    $processPresent = $false
    foreach ($entry in $processEntries) {
        if ($entry.TrimEnd('\').Equals($normalizedTarget, [System.StringComparison]::OrdinalIgnoreCase)) {
            $processPresent = $true
            break
        }
    }

    if (-not $processPresent) {
        $env:Path = "$PathValue;$env:Path"
    }
}

function Install-Binary {
    param(
        [Parameter(Mandatory = $true)][pscustomobject] $Manifest,
        [Parameter(Mandatory = $true)][string] $AssetKey,
        [Parameter(Mandatory = $true)][string] $TargetName,
        [Parameter(Mandatory = $true)][string] $TempRoot
    )

    $url = Get-ManifestLink -Manifest $Manifest -Key $AssetKey
    $destination = Join-Path $BinDir $TargetName
    $downloadPath = Join-Path $TempRoot $TargetName

    Write-Step "Downloading $TargetName"
    Download-FileWithProgress -Url $url -Destination $downloadPath -Activity "Downloading $TargetName"
    Copy-Item -Path $downloadPath -Destination $destination -Force
    Write-Success "Installed $TargetName -> $destination"
}

if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
    Fail 'LOCALAPPDATA is not set.'
}

$PreviaHome = Join-Path $env:LOCALAPPDATA 'Previa'
$BinDir = Join-Path $PreviaHome 'bin'

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($architecture -ne [System.Runtime.InteropServices.Architecture]::X64) {
    Fail "Unsupported Windows architecture: $architecture. Only amd64 is supported right now."
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("previa-install-" + [System.Guid]::NewGuid().ToString('N'))

try {
    Write-Step 'Previa installer'
    Write-Step 'Detecting platform'
    $osSlug = 'windows'
    $archSlug = 'amd64'
    Write-Success "Platform: $osSlug/$archSlug"

    Write-Step 'Downloading manifest'
    New-Item -Path $tempRoot -ItemType Directory -Force | Out-Null
    $manifestPath = Join-Path $tempRoot 'manifest.json'
    Download-FileWithProgress -Url $ManifestUrl -Destination $manifestPath -Activity 'Downloading manifest'
    $manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
    if (-not $manifest.version) {
        Fail 'Manifest is invalid: missing version.'
    }
    Write-Success "Resolved latest version $($manifest.version)"

    Write-Step "Installing binaries into $BinDir"
    New-Item -Path $BinDir -ItemType Directory -Force | Out-Null

    Install-Binary -Manifest $manifest -AssetKey "previa_main_${osSlug}_${archSlug}" -TargetName 'previa-main.exe' -TempRoot $tempRoot
    Install-Binary -Manifest $manifest -AssetKey "previa_runner_${osSlug}_${archSlug}" -TargetName 'previa-runner.exe' -TempRoot $tempRoot
    Install-Binary -Manifest $manifest -AssetKey "previactl_${osSlug}_${archSlug}" -TargetName 'previactl.exe' -TempRoot $tempRoot

    Write-Step 'Configuring PREVIA_HOME and PATH'
    [Environment]::SetEnvironmentVariable('PREVIA_HOME', $PreviaHome, 'User')
    $env:PREVIA_HOME = $PreviaHome
    Add-PathEntry -PathValue $BinDir

    Write-Success "Previa $($manifest.version) installed successfully."
    Write-Host "Installed directory: $PreviaHome" -ForegroundColor Blue
    Write-Host "Open a new terminal to use 'previactl' from PATH." -ForegroundColor Blue
}
catch {
    Fail $_.Exception.Message
}
finally {
    if (Test-Path $tempRoot) {
        Remove-Item -Path $tempRoot -Recurse -Force
    }
}
