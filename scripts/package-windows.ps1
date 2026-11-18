param(
    [string]$Binary = "$PSScriptRoot\..\target\x86_64-pc-windows-msvc\release\wolf3d.exe",
    [string]$Sdl2Dll = $env:SDL2_DLL_PATH,
    [string]$GameDataDir,
    [string]$DistDir = "$PSScriptRoot\..\dist"
)

$ErrorActionPreference = "Stop"
$Binary = [System.IO.Path]::GetFullPath($Binary)
$Sdl2Dll = if ([string]::IsNullOrWhiteSpace($Sdl2Dll)) { $null } else { [System.IO.Path]::GetFullPath($Sdl2Dll) }
if (!(Test-Path -LiteralPath $Binary -PathType Leaf)) { throw "wolf3d.exe not found: $Binary" }
if (!$Sdl2Dll) { throw "Set SDL2_DLL_PATH to the matching SDL2.dll before packaging." }
if (!(Test-Path -LiteralPath $Sdl2Dll -PathType Leaf)) { throw "SDL2.dll not found: $Sdl2Dll" }

$Root = (Resolve-Path "$PSScriptRoot\..").Path
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$Sdl2Dll = (Resolve-Path -LiteralPath $Sdl2Dll).Path
$GameDataDir = if ([string]::IsNullOrWhiteSpace($GameDataDir)) { $null } else { [System.IO.Path]::GetFullPath($GameDataDir) }
$Stage = Join-Path $DistDir "windows-x64"
$Archive = Join-Path $DistDir "wolf3d-windows-x64.zip"

if ((Split-Path $Sdl2Dll -Leaf) -ne "SDL2.dll") { throw "SDL2_DLL_PATH must point to SDL2.dll" }

New-Item -ItemType Directory -Force -Path $DistDir | Out-Null
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item $Binary (Join-Path $Stage "wolf3d.exe")
Copy-Item $Sdl2Dll (Join-Path $Stage "SDL2.dll")
Copy-Item (Join-Path $Root "README.rst") $Stage
Copy-Item (Join-Path $Root "PORTING.md") $Stage
Copy-Item (Join-Path $Root "WOLFSRC\README\LICENSE.DOC") (Join-Path $Stage "LICENSE.DOC")

if ($GameDataDir) {
    if (!(Test-Path -LiteralPath $GameDataDir -PathType Container)) { throw "Game-data directory not found: $GameDataDir" }
    $RequiredStems = @("GAMEMAPS", "MAPHEAD", "VSWAP", "VGAHEAD", "VGAGRAPH", "VGADICT", "AUDIOHED", "AUDIOT")
    $DataFiles = @(Get-ChildItem -LiteralPath $GameDataDir -File)
    $DataExtension = $null
    foreach ($Variant in @("WL6", "WL3", "WL1")) {
        $VariantFiles = @(
            foreach ($Stem in $RequiredStems) {
                $DataFiles | Where-Object {
                    [System.IO.Path]::GetFileNameWithoutExtension($_.Name) -ieq $Stem -and
                    [System.IO.Path]::GetExtension($_.Name) -ieq ".$Variant"
                } | Select-Object -First 1
            }
        )
        if ($VariantFiles.Count -eq $RequiredStems.Count) {
            $DataExtension = $Variant
            break
        }
    }
    if (!$DataExtension) { throw "Game-data directory must contain one complete WL6, WL3, or WL1 data set." }

    $DataStage = Join-Path $Stage "data"
    New-Item -ItemType Directory -Force -Path $DataStage | Out-Null
    foreach ($Stem in $RequiredStems) {
        $File = $DataFiles | Where-Object {
            [System.IO.Path]::GetFileNameWithoutExtension($_.Name) -ieq $Stem -and
            [System.IO.Path]::GetExtension($_.Name) -ieq ".$DataExtension"
        } | Select-Object -First 1
        Copy-Item -LiteralPath $File.FullName -Destination $DataStage
    }
}

if (Test-Path $Archive) { Remove-Item -Force $Archive }
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $Archive
Write-Output "Created $Archive"
