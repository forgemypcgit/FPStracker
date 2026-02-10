$ErrorActionPreference = 'Stop'

$Repo = if ($env:FPS_TRACKER_REPO) { $env:FPS_TRACKER_REPO } else { 'forgemypcgit/FPStracker' }
$BinaryName = 'fps-tracker.exe'
$InstallDir = if ($env:FPS_TRACKER_INSTALL_DIR) { $env:FPS_TRACKER_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'fps-tracker\bin' }
$RequestedVersion = if ($env:FPS_TRACKER_VERSION) { $env:FPS_TRACKER_VERSION } elseif ($args.Count -gt 0) { $args[0] } else { $null }
$CosignVersion = if ($env:FPS_TRACKER_COSIGN_VERSION) { $env:FPS_TRACKER_COSIGN_VERSION } else { 'v2.4.1' }
$CosignPubKeyOverride = if ($env:FPS_TRACKER_COSIGN_PUBKEY) { $env:FPS_TRACKER_COSIGN_PUBKEY } else { $null }
$SkipSignatureVerify = if ($env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY) { $env:FPS_TRACKER_SKIP_SIGNATURE_VERIFY } else { '0' }
$RequireSignatureVerify = if ($env:FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY) { $env:FPS_TRACKER_REQUIRE_SIGNATURE_VERIFY } else { '0' }
$SkipCosignChecksumVerify = if ($env:FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY) { $env:FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY } else { '0' }
$BaseUrlOverride = if ($env:FPS_TRACKER_BASE_URL) { $env:FPS_TRACKER_BASE_URL.TrimEnd('/') } else { $null }
$SkipPathUpdate = if ($env:FPS_TRACKER_SKIP_PATH_UPDATE) { $env:FPS_TRACKER_SKIP_PATH_UPDATE } else { '0' }

function Get-TargetTriple {
  if ($env:PROCESSOR_ARCHITECTURE -match 'ARM64') {
    throw 'Windows ARM64 binary is not published yet. Use x64 environment or build from source.'
  }

  return 'x86_64-pc-windows-msvc'
}

function Get-VersionTag([string]$ExplicitVersion, [string]$Repository) {
  if ($BaseUrlOverride) {
    if ($ExplicitVersion) {
      return $ExplicitVersion
    }
    return 'custom'
  }

  if ($ExplicitVersion) {
    return $ExplicitVersion
  }

  $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest"
  if (-not $latest.tag_name) {
    throw 'Could not resolve latest release tag. Set FPS_TRACKER_VERSION manually.'
  }

  return $latest.tag_name
}

function Verify-Checksum([string]$ArchivePath, [string]$ChecksumPath) {
  $expectedLine = (Get-Content -Path $ChecksumPath | Select-Object -First 1).Trim()
  if (-not $expectedLine) {
    throw 'Checksum file is empty.'
  }

  $expectedHash = ($expectedLine -split '\s+')[0].ToLower()
  $actualHash = (Get-FileHash -Algorithm SHA256 -Path $ArchivePath).Hash.ToLower()

  if ($expectedHash -ne $actualHash) {
    throw "Checksum mismatch for $ArchivePath"
  }
}

function Get-CosignExpectedSha256([string]$Version) {
  if ($Version -ne 'v2.4.1') {
    return $null
  }

  # Pinned checksum for cosign v2.4.1 (cosign-windows-amd64.exe).
  return '8d57f8a42a981d27290c4227271fa9f0f62ca6630eb4a21d316bd6b01405b87c'
}

function Ensure-Cosign([string]$TempDir, [string]$Version) {
  $existing = Get-Command cosign -ErrorAction SilentlyContinue
  if ($existing) {
    return $existing.Source
  }

  $cosignPath = Join-Path $TempDir 'cosign.exe'
  $url = "https://github.com/sigstore/cosign/releases/download/$Version/cosign-windows-amd64.exe"
  Invoke-WebRequest -Uri $url -OutFile $cosignPath

  $expected = Get-CosignExpectedSha256 -Version $Version
  if (-not $expected) {
    if ($SkipCosignChecksumVerify -eq '1') {
      Write-Warning 'cosign checksum verification skipped (FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY=1).'
    }
    else {
      throw "No pinned checksum available for cosign $Version. Set FPS_TRACKER_SKIP_COSIGN_CHECKSUM_VERIFY=1 to bypass."
    }
  }
  else {
    $actual = (Get-FileHash -Algorithm SHA256 -Path $cosignPath).Hash.ToLower()
    if ($actual -ne $expected) {
      throw "cosign checksum mismatch for cosign-windows-amd64.exe ($Version)."
    }
  }

  return $cosignPath
}

function Verify-Signature(
  [string]$CosignPath,
  [string]$ArchivePath,
  [string]$SignaturePath,
  [string]$PublicKeyPath
) {
  & $CosignPath verify-blob --key $PublicKeyPath --signature $SignaturePath $ArchivePath | Out-Null
  if ($LASTEXITCODE -ne 0) {
    throw "Signature verification failed for $ArchivePath"
  }
}

$target = Get-TargetTriple
$version = Get-VersionTag -ExplicitVersion $RequestedVersion -Repository $Repo
$assetName = "fps-tracker-$target.zip"
$baseUrl = if ($BaseUrlOverride) { $BaseUrlOverride } else { "https://github.com/$Repo/releases/download/$version" }

$tempDir = Join-Path $env:TEMP ("fps-tracker-install-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
  $archivePath = Join-Path $tempDir $assetName
  $checksumPath = Join-Path $tempDir ($assetName + '.sha256')
  $signaturePath = Join-Path $tempDir ($assetName + '.sig')
  $publicKeyPath = Join-Path $tempDir 'cosign.pub'

  Write-Host "Installing fps-tracker $version ($target)..."
  Invoke-WebRequest -Uri "$baseUrl/$assetName" -OutFile $archivePath
  Invoke-WebRequest -Uri "$baseUrl/$assetName.sha256" -OutFile $checksumPath

  if ($SkipSignatureVerify -ne '1') {
    try {
      Invoke-WebRequest -Uri "$baseUrl/$assetName.sig" -OutFile $signaturePath
      $effectivePubKey = $publicKeyPath
      if ($CosignPubKeyOverride) {
        if (-not (Test-Path -Path $CosignPubKeyOverride)) {
          throw "COSIGN pubkey override not found: $CosignPubKeyOverride"
        }
        $effectivePubKey = $CosignPubKeyOverride
      }
      else {
        Invoke-WebRequest -Uri "$baseUrl/cosign.pub" -OutFile $publicKeyPath
      }
      $cosignPath = Ensure-Cosign -TempDir $tempDir -Version $CosignVersion
      Verify-Signature -CosignPath $cosignPath -ArchivePath $archivePath -SignaturePath $signaturePath -PublicKeyPath $effectivePubKey
      Write-Host 'Signature verification succeeded.'
    }
    catch {
      if ($RequireSignatureVerify -eq '1') {
        throw 'Signature assets not available for this release. Set FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1 to bypass.'
      }
      Write-Warning 'Signature assets not available for this release; skipping signature verification.'
    }
  }
  else {
    Write-Warning 'Signature verification skipped (FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1).'
  }

  Verify-Checksum -ArchivePath $archivePath -ChecksumPath $checksumPath

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
  Copy-Item -Path (Join-Path $tempDir $BinaryName) -Destination (Join-Path $InstallDir $BinaryName) -Force

  if ($SkipPathUpdate -ne '1') {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$InstallDir*") {
      $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
      [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
      Write-Host "Added $InstallDir to user PATH. Restart terminal to use command globally."
    }
  }
  else {
    Write-Host 'PATH update skipped (FPS_TRACKER_SKIP_PATH_UPDATE=1).'
  }

  Write-Host "Installed to: $(Join-Path $InstallDir $BinaryName)"
  Write-Host "Run: fps-tracker --help"
}
finally {
  Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
