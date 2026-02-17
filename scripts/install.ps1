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
  $archHints = @($env:PROCESSOR_ARCHITECTURE, $env:PROCESSOR_ARCHITEW6432) -join ';'
  if ($archHints -match 'ARM64') {
    throw 'Windows ARM64 binary is not published yet. Use x64 environment or build from source.'
  }

  if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'Windows x86 is not supported. Use a 64-bit Windows environment.'
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

  $headers = @{
    'User-Agent' = 'fps-tracker-installer'
    'Accept' = 'application/vnd.github+json'
  }

  $tag = $null
  try {
    $latest = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases/latest" -Headers $headers
    if ($latest -and $latest.tag_name) {
      $tag = $latest.tag_name
    }
  } catch {
    $tag = $null
  }

  if (-not $tag) {
    # GitHub API can be rate-limited; fall back to resolving the final redirect URL of releases/latest.
    try {
      $page = Invoke-WebRequest -Uri "https://github.com/$Repository/releases/latest" -Headers @{ 'User-Agent' = 'fps-tracker-installer' } -TimeoutSec 45 -UseBasicParsing
      $final = $page.BaseResponse.ResponseUri.AbsoluteUri
      if ($final) {
        $tag = ($final -split '/')[-1]
      }
    } catch {
      $tag = $null
    }
  }

  if (-not $tag -or ($tag -notmatch '^v[0-9]')) {
    throw 'Could not resolve latest release tag. Set FPS_TRACKER_VERSION manually.'
  }

  return $tag
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

function Invoke-DownloadWithRetry(
  [string]$Uri,
  [string]$OutFile,
  [int]$MaxAttempts = 4,
  [int]$TimeoutSec = 90
) {
  $lastError = $null
  for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
    try {
      Invoke-WebRequest -Uri $Uri -OutFile $OutFile -TimeoutSec $TimeoutSec
      return
    }
    catch {
      $lastError = $_
      if (Test-Path -Path $OutFile) {
        Remove-Item -Path $OutFile -Force -ErrorAction SilentlyContinue
      }
      if ($attempt -lt $MaxAttempts) {
        Start-Sleep -Seconds ([Math]::Min([Math]::Pow(2, $attempt), 10))
      }
    }
  }

  throw "Failed to download $Uri after $MaxAttempts attempt(s): $lastError"
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
  Invoke-DownloadWithRetry -Uri $url -OutFile $cosignPath

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

function Write-EmbeddedCosignPubKey([string]$Path) {
@'
-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE0rofgmxGgaiHuKApwl63oDguA3na
QizcDZ/PzVl4/g+G3ph22YGwK+ABID5qtjK827+G1wfjVrq6LUSlIkFfgQ==
-----END PUBLIC KEY-----
'@ | Out-File -FilePath $Path -Encoding ascii
}

$target = Get-TargetTriple
$version = Get-VersionTag -ExplicitVersion $RequestedVersion -Repository $Repo
$zipAssetName = "fps-tracker-$target.zip"
$exeAssetName = "fps-tracker-$target.exe"
$baseUrl = if ($BaseUrlOverride) { $BaseUrlOverride } else { "https://github.com/$Repo/releases/download/$version" }

$tempDir = Join-Path $env:TEMP ("fps-tracker-install-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempDir | Out-Null

try {
  $downloadAssetName = $zipAssetName
  try {
    # Prefer a direct .exe if available. It's simpler for users and avoids archive extraction.
    # If HEAD isn't permitted in a given network environment, we fall back to the zip.
    Invoke-WebRequest -Uri "$baseUrl/$exeAssetName" -Method Head -TimeoutSec 20 | Out-Null
    $downloadAssetName = $exeAssetName
  }
  catch {
    $downloadAssetName = $zipAssetName
  }

  $archivePath = Join-Path $tempDir $downloadAssetName
  $checksumPath = Join-Path $tempDir ($downloadAssetName + '.sha256')
  $signaturePath = Join-Path $tempDir ($downloadAssetName + '.sig')
  $publicKeyPath = Join-Path $tempDir 'cosign.pub'

  Write-Host "Installing fps-tracker $version ($target)..."
  Invoke-DownloadWithRetry -Uri "$baseUrl/$downloadAssetName" -OutFile $archivePath
  Invoke-DownloadWithRetry -Uri "$baseUrl/$downloadAssetName.sha256" -OutFile $checksumPath

  if ($SkipSignatureVerify -ne '1') {
    $signatureDownloaded = $false
    try {
      Invoke-DownloadWithRetry -Uri "$baseUrl/$downloadAssetName.sig" -OutFile $signaturePath
      $signatureDownloaded = $true
    }
    catch {
      if ($RequireSignatureVerify -eq '1') {
        throw 'Signature assets not available for this release. Set FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1 to bypass.'
      }
      Write-Warning 'Signature assets not available for this release; skipping signature verification.'
    }

    if ($signatureDownloaded) {
      $effectivePubKey = $publicKeyPath
      if ($CosignPubKeyOverride) {
        if (-not (Test-Path -Path $CosignPubKeyOverride)) {
          throw "COSIGN pubkey override not found: $CosignPubKeyOverride"
        }
        $effectivePubKey = $CosignPubKeyOverride
      }
      else {
        # Default to a pinned public key embedded in this installer, so signature verification
        # doesn't rely on downloading a mutable key from the release itself.
        Write-EmbeddedCosignPubKey -Path $publicKeyPath
      }
      $cosignPath = Ensure-Cosign -TempDir $tempDir -Version $CosignVersion
      Verify-Signature -CosignPath $cosignPath -ArchivePath $archivePath -SignaturePath $signaturePath -PublicKeyPath $effectivePubKey
      Write-Host 'Signature verification succeeded.'
    }
  }
  else {
    Write-Warning 'Signature verification skipped (FPS_TRACKER_SKIP_SIGNATURE_VERIFY=1).'
  }

  Verify-Checksum -ArchivePath $archivePath -ChecksumPath $checksumPath

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  if ($downloadAssetName -eq $exeAssetName) {
    Copy-Item -Path $archivePath -Destination (Join-Path $InstallDir $BinaryName) -Force
  }
  else {
    Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
    $extractedBinary = Join-Path $tempDir $BinaryName
    if (-not (Test-Path -Path $extractedBinary)) {
      throw "Extracted archive did not contain expected binary: $BinaryName"
    }
    Copy-Item -Path $extractedBinary -Destination (Join-Path $InstallDir $BinaryName) -Force
  }

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
