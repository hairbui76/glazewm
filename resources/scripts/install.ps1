# One-line installer for GlazeWM on Windows.
#
# Usage:
#   irm https://raw.githubusercontent.com/hairbui76/glazewm/main/resources/scripts/install.ps1 | iex
#
# Configurable through environment variables, since arguments can't be
# passed to a script that's piped into `iex`:
#   $env:GLAZEWM_VERSION = '3.11.0'          # Version to install. Defaults to latest.
#   $env:GLAZEWM_REPO    = 'owner/repo'      # Repository to install from.
#   $env:GLAZEWM_SILENT  = '1'               # Install without any installer UI ('1'/'true'/'yes').

#Requires -Version 5.1

$ErrorActionPreference = 'Stop'

function Install-GlazeWM {
  [CmdletBinding()]
  param(
    # Repository to resolve releases from, in `owner/repo` format.
    [string]$Repo = $(
      if ($env:GLAZEWM_REPO) { $env:GLAZEWM_REPO } else { 'hairbui76/glazewm' }
    ),

    # Version to install (e.g. `3.11.0`). Defaults to the latest release.
    [string]$Version = $env:GLAZEWM_VERSION,

    # Whether to run the installer without any UI.
    [switch]$Silent = ($env:GLAZEWM_SILENT -in @('1', 'true', 'yes'))
  )

  # Older PowerShell hosts default to TLS 1.0, which GitHub rejects.
  [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

  # Progress rendering makes `Invoke-WebRequest` downloads an order of
  # magnitude slower.
  $originalProgress = $ProgressPreference
  $ProgressPreference = 'SilentlyContinue'

  $downloadDir = Join-Path $env:TEMP 'glazewm-install'
  $installerPath = $null

  try {
    $release = Get-GlazeWMRelease -Repo $Repo -Version $Version
    $asset = $release.assets |
      Where-Object { $_.name -like 'glazewm-v*.exe' } |
      Select-Object -First 1

    if (!$asset) {
      throw "Release $($release.tag_name) has no Windows installer attached."
    }

    Write-Host "Installing GlazeWM $($release.tag_name)." -ForegroundColor Cyan

    New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
    $installerPath = Join-Path $downloadDir $asset.name

    Write-Host "Downloading $($asset.name) ($([math]::Round($asset.size / 1MB, 1)) MB)."
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $installerPath

    # The installer is per-machine, so it self-elevates through UAC.
    $installerArgs = if ($Silent) { @('/quiet', '/norestart') } else { @('/passive', '/norestart') }

    Write-Host 'Running installer. Accept the UAC prompt to continue.'
    $process = Start-Process -FilePath $installerPath -ArgumentList $installerArgs -PassThru -Wait

    switch ($process.ExitCode) {
      0 {
        Write-Host "GlazeWM $($release.tag_name) installed." -ForegroundColor Green
      }
      3010 {
        Write-Host "GlazeWM $($release.tag_name) installed. Restart to complete setup." -ForegroundColor Green
      }
      default {
        throw "Installer exited with code $($process.ExitCode)."
      }
    }

    Write-Host ''
    Write-Host 'Launch it from the Start menu, or run `glazewm start` in a new terminal.'
  }
  finally {
    $ProgressPreference = $originalProgress

    if ($installerPath -and (Test-Path $installerPath)) {
      Remove-Item -Path $installerPath -Force -ErrorAction SilentlyContinue
    }
  }
}

function Get-GlazeWMRelease {
  [CmdletBinding()]
  param(
    # Repository to resolve the release from, in `owner/repo` format.
    [Parameter(Mandatory)]
    [string]$Repo,

    # Version to resolve. Resolves the latest release when empty.
    [string]$Version
  )

  $url = if ($Version) {
    "https://api.github.com/repos/$Repo/releases/tags/v$($Version.TrimStart('v'))"
  } else {
    "https://api.github.com/repos/$Repo/releases/latest"
  }

  $headers = @{
    'Accept' = 'application/vnd.github+json'
    'User-Agent' = 'glazewm-install'
    'X-GitHub-Api-Version' = '2022-11-28'
  }

  try {
    Invoke-RestMethod -Uri $url -Headers $headers
  }
  catch {
    throw "Unable to resolve a GlazeWM release from '$Repo'. $($_.Exception.Message)"
  }
}

Install-GlazeWM
