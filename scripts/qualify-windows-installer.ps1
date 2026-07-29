param(
  [Parameter(Mandatory = $true)]
  [string]$ContractPath,
  [Parameter(Mandatory = $true)]
  [string]$ArtifactsRoot,
  [Parameter(Mandatory = $true)]
  [string]$EvidencePath,
  [Parameter(Mandatory = $true)]
  [string]$Repository
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$contract = Get-Content -LiteralPath $ContractPath -Raw | ConvertFrom-Json
$release = $contract.release
$version = $release.tag.TrimStart("v")
$windows = $release.windows
$expectedMsi = Join-Path $ArtifactsRoot ("Whisper_Input_{0}_x64_en-US.msi" -f $version)
$expectedSetup = Join-Path $ArtifactsRoot ("Whisper_Input_{0}_x64_setup.exe" -f $version)
$evidence = [ordered]@{
  schemaVersion = 1
  releaseTag = $release.tag
  sourceCommit = $release.sourceCommit
  windowsTarget = $windows.target
  upgradeBaseline = $windows.upgradeBaseline
  startedAtUtc = (Get-Date).ToUniversalTime().ToString("o")
  status = "running"
  checks = @()
}

function Add-Check {
  param(
    [string]$Name,
    [string]$Status,
    [string]$Detail
  )
  $script:evidence.checks += [ordered]@{
    name = $Name
    status = $Status
    detail = $Detail
    atUtc = (Get-Date).ToUniversalTime().ToString("o")
  }
}

function Write-Evidence {
  $evidence.completedAtUtc = (Get-Date).ToUniversalTime().ToString("o")
  $directory = Split-Path -Parent $EvidencePath
  New-Item -ItemType Directory -Force -Path $directory | Out-Null
  $evidence | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $EvidencePath -Encoding utf8
}

function Invoke-Msi {
  param(
    [ValidateSet("install", "uninstall")]
    [string]$Action,
    [string]$MsiPath
  )
  $flag = if ($Action -eq "install") { "/i" } else { "/x" }
  $arguments = "$flag `"$MsiPath`" /qn /norestart REBOOT=ReallySuppress"
  $process = Start-Process -FilePath "msiexec.exe" -ArgumentList $arguments -Wait -PassThru
  if ($process.ExitCode -notin @(0, 3010)) {
    throw "MSI $Action failed with exit code $($process.ExitCode): $MsiPath"
  }
  return $process.ExitCode
}

function Get-AppRegistration {
  param([string]$ProductName)
  $roots = @(
    "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
  )
  $entries = foreach ($root in $roots) {
    if (Test-Path -LiteralPath $root) {
      Get-ChildItem -LiteralPath $root | ForEach-Object {
        $item = Get-ItemProperty -LiteralPath $_.PSPath
        if ($item.DisplayName -eq $ProductName) {
          $item
        }
      }
    }
  }
  return @($entries | Sort-Object DisplayVersion -Descending | Select-Object -First 1)
}

function Get-InstalledBinary {
  param(
    [object]$Registration,
    [string]$BinaryName
  )
  $roots = @()
  if ($Registration.InstallLocation -and (Test-Path -LiteralPath $Registration.InstallLocation)) {
    $roots += $Registration.InstallLocation
  }
  $roots += @(
    (Join-Path $env:ProgramFiles "Whisper Input"),
    (Join-Path ${env:ProgramFiles(x86)} "Whisper Input")
  )
  foreach ($root in ($roots | Select-Object -Unique)) {
    if (-not (Test-Path -LiteralPath $root)) {
      continue
    }
    $binary = Get-ChildItem -LiteralPath $root -Recurse -File -Filter $BinaryName -ErrorAction SilentlyContinue |
      Select-Object -First 1
    if ($binary) {
      return $binary.FullName
    }
  }
  throw "Installed application binary $BinaryName was not found."
}

function Test-InstalledAppLaunch {
  param(
    [object]$Registration,
    [string]$BinaryName,
    [string]$Channel
  )
  $binary = Get-InstalledBinary -Registration $Registration -BinaryName $BinaryName
  $process = Start-Process -FilePath $binary -PassThru
  Start-Sleep -Seconds 4
  $process.Refresh()
  if ($process.HasExited) {
    throw "$Channel installed app exited before the smoke check completed (exit code $($process.ExitCode))."
  }
  Stop-Process -Id $process.Id -Force
  Add-Check -Name "$Channel-launch" -Status "passed" -Detail "Launched $binary and observed a live process."
}

function Invoke-NsisUninstall {
  param([object]$Registration)
  $uninstall = [string]$Registration.UninstallString
  $pathMatch = [regex]::Match($uninstall, '^"(?<path>[^"]+)"')
  $uninstaller = if ($pathMatch.Success) {
    $pathMatch.Groups["path"].Value
  } elseif ($Registration.InstallLocation) {
    Join-Path $Registration.InstallLocation "uninstall.exe"
  } else {
    Join-Path $env:ProgramFiles "Whisper Input\uninstall.exe"
  }
  if (-not (Test-Path -LiteralPath $uninstaller)) {
    throw "NSIS uninstaller was not found: $uninstaller"
  }
  $process = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
  if ($process.ExitCode -ne 0) {
    throw "NSIS uninstall failed with exit code $($process.ExitCode)."
  }
}

function Assert-Removed {
  param([string]$ProductName)
  Start-Sleep -Seconds 2
  if ((Get-AppRegistration -ProductName $ProductName).Count -ne 0) {
    throw "$ProductName is still registered after uninstall."
  }
}

try {
  foreach ($path in @($expectedMsi, $expectedSetup)) {
    if (-not (Test-Path -LiteralPath $path)) {
      throw "Required Windows installer is missing: $path"
    }
  }
  Add-Check -Name "artifact-presence" -Status "passed" -Detail "MSI and NSIS installers were present."

  $previousDir = Join-Path ([System.IO.Path]::GetTempPath()) "whisper-input-upgrade-baseline"
  Remove-Item -LiteralPath $previousDir -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force -Path $previousDir | Out-Null
  & gh release download $windows.upgradeBaseline.tag --repo $Repository --pattern $windows.upgradeBaseline.msiAsset --dir $previousDir
  if ($LASTEXITCODE -ne 0) {
    throw "Could not download the contract's Windows upgrade baseline."
  }
  $previousMsi = Join-Path $previousDir $windows.upgradeBaseline.msiAsset
  if (-not (Test-Path -LiteralPath $previousMsi)) {
    throw "Downloaded upgrade baseline is missing: $previousMsi"
  }

  Invoke-Msi -Action install -MsiPath $previousMsi | Out-Null
  Add-Check -Name "upgrade-baseline-install" -Status "passed" -Detail "Installed $($windows.upgradeBaseline.tag) MSI."

  Invoke-Msi -Action install -MsiPath $expectedMsi | Out-Null
  $registration = Get-AppRegistration -ProductName $release.productName
  if ($registration.Count -ne 1 -or $registration[0].DisplayVersion -ne $version) {
    throw "MSI upgrade did not register $($release.productName) version $version."
  }
  Add-Check -Name "msi-upgrade" -Status "passed" -Detail "Upgraded from $($windows.upgradeBaseline.tag) to $version."

  Test-InstalledAppLaunch -Registration $registration[0] -BinaryName "whisper-input.exe" -Channel "msi"
  Invoke-Msi -Action uninstall -MsiPath $expectedMsi | Out-Null
  Assert-Removed -ProductName $release.productName
  Add-Check -Name "msi-uninstall" -Status "passed" -Detail "MSI uninstall removed the product registration."

  $setupProcess = Start-Process -FilePath $expectedSetup -ArgumentList "/S" -Wait -PassThru
  if ($setupProcess.ExitCode -ne 0) {
    throw "NSIS install failed with exit code $($setupProcess.ExitCode)."
  }
  $registration = Get-AppRegistration -ProductName $release.productName
  if ($registration.Count -ne 1 -or $registration[0].DisplayVersion -ne $version) {
    throw "NSIS install did not register $($release.productName) version $version."
  }
  Add-Check -Name "nsis-install" -Status "passed" -Detail "Installed the NSIS setup executable silently."
  Test-InstalledAppLaunch -Registration $registration[0] -BinaryName "whisper-input.exe" -Channel "nsis"
  Invoke-NsisUninstall -Registration $registration[0]
  Assert-Removed -ProductName $release.productName
  Add-Check -Name "nsis-uninstall" -Status "passed" -Detail "NSIS uninstall removed the product registration."

  $evidence.status = "passed"
} catch {
  $evidence.status = "failed"
  $evidence.error = $_.Exception.Message
  throw
} finally {
  Write-Evidence
}
