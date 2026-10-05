$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path -Parent $PSScriptRoot
$fixture = Join-Path ([IO.Path]::GetTempPath()) ('mist-choco-test-' + [guid]::NewGuid())
$digest = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
$mistTest = [pscustomobject]@{
    Signer = 'A1' * 32
    Contents = 'abc'
    SignatureStatus = [System.Management.Automation.SignatureStatus]::Valid
    ActualSigner = 'A1' * 32
    Installed = $false
    Version = '0.1.0'
}

# Only system boundaries are substituted: download, Windows trust store, and
# installer execution. The generated checksum/trust decisions run unchanged.
function Get-ChocolateyWebFile {
    param($PackageName, $FileFullPath, $Url64bit, $Checksum64, $ChecksumType64)
    if ($Url64bit -ne "https://github.com/mindful-time/Mist/releases/download/v$($mistTest.Version)/Mist-windows-x86_64-setup.exe") { throw 'Wrong URL' }
    if ($Checksum64 -ne $digest -or $ChecksumType64 -ne 'sha256') { throw 'Missing pinned checksum' }
    [IO.File]::WriteAllText($FileFullPath, $mistTest.Contents, [Text.UTF8Encoding]::new($false))
}
function Get-AuthenticodeSignature {
    param($FilePath)
    $certificate = [pscustomobject]@{ Fingerprint = $mistTest.ActualSigner }
    $certificate | Add-Member ScriptMethod GetCertHashString { param($algorithm) return $this.Fingerprint }
    return [pscustomobject]@{ Status = $mistTest.SignatureStatus; SignerCertificate = $certificate }
}
function Install-ChocolateyInstallPackage {
    param($PackageName, $FileType, $SilentArgs, $File64, $ValidExitCodes)
    if ($PackageName -ne 'mist-tts' -or $FileType -ne 'exe' -or $SilentArgs -ne '/S' -or $ValidExitCodes[0] -ne 0) { throw 'Wrong installer arguments' }
    $mistTest.Installed = $true
}

try {
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $names = @('Mist-macos-aarch64.dmg', 'Mist-macos-x86_64.dmg', 'Mist-windows-x86_64-setup.exe')
    foreach ($name in $names) { [IO.File]::WriteAllText((Join-Path $fixture $name), 'abc', [Text.UTF8Encoding]::new($false)) }
    [IO.File]::WriteAllText((Join-Path $fixture 'SHA256SUMS'), (($names | ForEach-Object { "$digest  $_" }) -join "`n") + "`n")
    foreach ($version in @('0.1.0', '0.1.0-rc.1')) {
        $mistTest.Version = $version
        $output = Join-Path $fixture "packages-$version"
        & node (Join-Path $PSScriptRoot 'build-package-managers.mjs') $version $fixture $output $mistTest.Signer
        if ($LASTEXITCODE -ne 0) { throw "Recipe generation failed: $version" }
        $install = Join-Path $output 'chocolatey/tools/chocolateyinstall.ps1'
        foreach ($scenario in @('corrupt', 'unsigned', 'wrong-publisher', 'valid')) {
            $mistTest.Contents = if ($scenario -eq 'corrupt') { 'corrupt' } else { 'abc' }
            $mistTest.SignatureStatus = if ($scenario -eq 'unsigned') { [System.Management.Automation.SignatureStatus]::NotSigned } else { [System.Management.Automation.SignatureStatus]::Valid }
            $mistTest.ActualSigner = if ($scenario -eq 'wrong-publisher') { 'B2' * 32 } else { $mistTest.Signer }
            $mistTest.Installed = $false
            $rejected = $false
            $reason = ''
            try { & $install } catch { $rejected = $true; $reason = $_.Exception.Message }
            if ($scenario -eq 'valid') {
                if ($rejected -or -not $mistTest.Installed) { throw "Valid installer was not accepted: ${version}: $reason" }
            } elseif (-not $rejected -or $mistTest.Installed) { throw "Unsafe installer accepted: ${version}: $scenario" }
            if (Test-Path (Join-Path (Split-Path $install) 'Mist-windows-x86_64-setup.exe')) { throw 'Temporary installer was not removed' }
        }
        if ($env:OS -eq 'Windows_NT') {
            & choco pack (Join-Path $output 'chocolatey/mist-tts.nuspec') --outputdirectory $fixture
            if ($LASTEXITCODE -ne 0 -or -not (Test-Path (Join-Path $fixture "mist-tts.$version.nupkg"))) { throw "Chocolatey package creation failed: $version" }
        }
    }
    Write-Host 'Chocolatey recipe, checksum, unsigned/wrong-publisher rejection, and package tests passed.'
} finally {
    if (Test-Path $fixture) { Remove-Item -LiteralPath $fixture -Recurse -Force }
}
