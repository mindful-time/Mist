$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repository = if ($env:MIST_REPOSITORY) { $env:MIST_REPOSITORY } else { "mindful-time/Mist" }
$release = if ($env:MIST_RELEASE) { $env:MIST_RELEASE } else { "latest" }
$releaseBaseUrl = $env:MIST_RELEASE_BASE_URL
$downloadOnly = $env:MIST_DOWNLOAD_ONLY -eq "1"
$expectedSignerSha256 = if ($env:MIST_EXPECTED_WINDOWS_SIGNER_SHA256) {
    $env:MIST_EXPECTED_WINDOWS_SIGNER_SHA256
} else {
    "__MIST_WINDOWS_SIGNER_SHA256__"
}

if (-not $releaseBaseUrl) {
    if ($release -eq "latest") {
        $releasePath = "latest/download"
    } else {
        $tag = if ($release.StartsWith("v")) { $release } else { "v$release" }
        $releasePath = "download/$tag"
    }
    $releaseBaseUrl = "https://github.com/$repository/releases/$releasePath"
}
$releaseBaseUrl = $releaseBaseUrl.TrimEnd("/")

$artifact = "Mist-windows-x86_64-setup.exe"
$temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("mist-installer-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null

try {
    $checksumsPath = Join-Path $temporaryDirectory "SHA256SUMS"
    $artifactPath = Join-Path $temporaryDirectory $artifact
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseBaseUrl/SHA256SUMS" -OutFile $checksumsPath
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseBaseUrl/$artifact" -OutFile $artifactPath

    $escapedArtifact = [regex]::Escape($artifact)
    $checksumLine = Get-Content $checksumsPath | Where-Object { $_ -match "^([a-fA-F0-9]{64})\s+\*?$escapedArtifact$" } | Select-Object -First 1
    if (-not $checksumLine) {
        throw "SHA256SUMS does not contain $artifact."
    }
    $expectedChecksum = ($checksumLine -split "\s+")[0].ToLowerInvariant()
    $actualChecksum = (Get-FileHash -Algorithm SHA256 $artifactPath).Hash.ToLowerInvariant()
    if ($actualChecksum -ne $expectedChecksum) {
        throw "Checksum verification failed for $artifact."
    }

    $signature = Get-AuthenticodeSignature $artifactPath
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "Authenticode verification failed for $artifact: $($signature.StatusMessage)"
    }
    if ($expectedSignerSha256.StartsWith("__MIST_")) {
        throw "The Mist installer is missing its Windows signing identity."
    }
    $actualSignerSha256 = $signature.SignerCertificate.GetCertHashString(
        [System.Security.Cryptography.HashAlgorithmName]::SHA256
    )
    if ($actualSignerSha256 -ne $expectedSignerSha256) {
        throw "The downloaded installer is not signed by the expected Windows certificate."
    }

    if ($downloadOnly) {
        $destination = if ($env:MIST_DOWNLOAD_DIR) { $env:MIST_DOWNLOAD_DIR } else { (Get-Location).Path }
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        $downloadPath = Join-Path $destination $artifact
        Copy-Item $artifactPath $downloadPath
        Write-Host "Downloaded and verified $downloadPath"
    } else {
        Write-Host "Downloaded and verified $artifact. Starting the installer..."
        $process = Start-Process -FilePath $artifactPath -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "Mist installer exited with code $($process.ExitCode)."
        }
    }
} finally {
    Remove-Item -Recurse -Force $temporaryDirectory -ErrorAction SilentlyContinue
}
