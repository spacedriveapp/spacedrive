Set-PSDebug -Trace 1

# Enables strict mode, which causes PowerShell to treat uninitialized variables, undefined functions, and other common errors as terminating errors.
$ErrorActionPreference = if ($env:CI) { 'Stop' } else { 'Inquire' }
Set-StrictMode -Version Latest

function Reset-Path {
    $env:Path = [System.Environment]::ExpandEnvironmentVariables(
        [System.Environment]::GetEnvironmentVariable('Path', 'Machine') +
        [IO.Path]::PathSeparator +
        [System.Environment]::GetEnvironmentVariable('Path', 'User')
    )
}

# Verify if environment is Windows 64-bit and if the user is an administrator
if ((-not [string]::IsNullOrEmpty($env:PROCESSOR_ARCHITEW6432)) -or (
        "$env:PROCESSOR_ARCHITECTURE" -eq 'ARM64'
    ) -or (
        -not [System.Environment]::Is64BitOperatingSystem
        # Powershell >= 6 is cross-platform, check if running on Windows
    ) -or (($PSVersionTable.PSVersion.Major -ge 6) -and (-not $IsWindows))
) {
    $ErrorActionPreference = 'Continue'
    Write-Host # There is no oficial ffmpeg binaries for Windows 32 or ARM
    if (Test-Path "$($env:WINDIR)\SysNative\WindowsPowerShell\v1.0\powershell.exe" -PathType Leaf) {
        throw 'You are using PowerShell (32-bit), please re-run in PowerShell (64-bit)'
    } else {
        throw 'This script is only supported on Windows 64-bit'
    }
    Exit 1
} elseif (
    -not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
) {
    # Start a new PowerShell process with administrator privileges and set the working directory to the directory where the script is located
    Start-Process -Wait -FilePath 'PowerShell.exe' -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Definition)`"" -WorkingDirectory "$PSScriptRoot"
    # NOTICE: Any modified environment variables should be reloaded here, so the user doesn't have to restart the shell after running the script
    Reset-Path
    Exit
}

function Exit-WithError($err, $help = $null) {
    if ($null -ne $help) {
        Write-Host
        Write-Host $help -ForegroundColor DarkRed
    }
    throw $err
    Exit 1
}

function Add-DirectoryToPath($directory) {
    if ($env:Path.Split(';') -notcontains $directory) {
        [System.Environment]::SetEnvironmentVariable(
            'Path',
            [System.Environment]::GetEnvironmentVariable('Path', 'User') + [IO.Path]::PathSeparator + $directory,
            'User'
        )

        if ($env:CI) {
            # If running in CI, we need to use GITHUB_PATH instead of the normal PATH env variables
            Add-Content $env:GITHUB_PATH "$directory`n"
        }
    }

    # The following is needed to make the environment variable available to the current PowerShell process
    Reset-Path
}

function Invoke-RestMethodGithub {
    [CmdletBinding()]
    param (
        [Parameter(Mandatory = $true)]
        [string]$Uri,
        [string]$Method = 'GET',
        [hashtable]$Headers = @{},
        [string]$UserAgent = 'PowerShell'
    )

    $headers.Add('Accept', 'application/vnd.github+json')
    $headers.Add('X-GitHub-Api-Version', '2022-11-28')

    if (![string]::IsNullOrEmpty($env:GITHUB_TOKEN)) {
        $headers.Add('Authorization', "Bearer $($env:GITHUB_TOKEN)")
    }

    $params = @{
        Uri       = $Uri
        Method    = $Method
        Headers   = $Headers
        UserAgent = $UserAgent
    }

    Invoke-RestMethod @params
}

# Reset PATH to ensure the script doesn't have stale Path entries
Reset-Path

# Get temp folder
$temp = [System.IO.Path]::GetTempPath()

# Get project dir (get grandparent dir from script location: <PROJECT_ROOT>\.github\scripts)
$projectRoot = Split-Path -Path (Split-Path -Path $PSScriptRoot -Parent) -Parent

# Pnpm
$pnpm_major = '8'

# Currently LLVM >= 16 is not supported due to incompatibilities with ffmpeg-sys-next
# See https://github.com/spacedriveapp/spacedrive/issues/677
$llvm_major = '15'

# Change CWD to project root
Set-Location $projectRoot
Remove-Item -Path "$projectRoot\.cargo\config" -Force -ErrorAction SilentlyContinue
Remove-Item -Path "$projectRoot\target\Frameworks" -Recurse -Force -ErrorAction SilentlyContinue

Write-Host 'Spacedrive Development Environment Setup' -ForegroundColor Magenta
Write-Host @"

To set up your machine for Spacedrive development, this script will do the following:
1) Install Windows C++ build tools
2) Install Edge Webview 2
3) Install Rust and Cargo
4) Install Rust tools
5) Install Node.js, npm and pnpm $pnpm_major
6) Install LLVM $llvm_major (compiler for ffmpeg-rust)
7) Download the protbuf compiler
8) Download a compatible ffmpeg build
"@

# Install C++ build tools and Rust
# GitHub Actions already has all of this installed
if (-not $env:CI) {
    # Check connectivity to GitHub
    $ProgressPreference = 'SilentlyContinue'
    if (-not ((Test-NetConnection -ComputerName 'github.com' -Port 80).TcpTestSucceeded)) {
        throw "Can't connect to github, maybe internet is down?"
    }
    $ProgressPreference = 'Continue'

    if (-not (Get-Command winget -ea 0)) {
        Exit-WithError 'winget not available' @'
Follow the instructions here to install winget:
https://learn.microsoft.com/windows/package-manager/winget/
'@
    }

    Write-Host
    Write-Host 'Installing Visual Studio Build Tools...' -ForegroundColor Yellow
    Write-Host 'This will take some time as it involves downloading several gigabytes of data....' -ForegroundColor Cyan
    # Force install because BuildTools is itself a package manager, so let it decide if something needs to be installed or not
    winget install --exact --no-upgrade --accept-source-agreements --force --disable-interactivity --id Microsoft.VisualStudio.2022.BuildTools --override '--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'

    Write-Host
    Write-Host 'Installing Edge Webview 2...' -ForegroundColor Yellow
    try {
        # This is normally already available, but on some early Windows 10 versions it isn't
        winget install --exact --no-upgrade --accept-source-agreements --disable-interactivity --id Microsoft.EdgeWebView2Runtime
    } catch {}

    Write-Host
    Write-Host 'Installing Rust and Cargo...' -ForegroundColor Yellow
    try {
        winget install --exact --no-upgrade --accept-source-agreements --disable-interactivity --id Rustlang.Rustup
        Reset-Path # Reset Path to ensure that cargo is available to the command bellow
    } catch {}

    Write-Host
    Write-Host 'Installing Rust tools' -ForegroundColor Yellow
    cargo install cargo-watch
}

Write-Host
Write-Host 'Checking for pnpm...' -ForegroundColor Yellow
if ($env:CI) {
    # The CI has pnpm installed already
    Write-Host 'Running with CI, skipping pnpm install.' -ForegroundColor Green
} elseif ((Get-Command pnpm -ea 0) -and (pnpm --version | Select-Object -First 1) -match "^$pnpm_major\." ) {
    Write-Host "pnpm $pnpm_major is installed." -ForegroundColor Green
} else {
    # Check for pnpm installed with standalone installer
    if (($null -ne $env:PNPM_HOME) -and (Test-Path "$env:PNPM_HOME/pnpm.exe" -PathType Leaf)) {
        Exit-WithError 'You have a incompatible version of pnpm installed, please remove it and run this script again' @'
Follow the instructions here to uninstall pnpm:
https://pnpm.io/uninstall
'@
    } else {
        # Remove possible remaining envvars from old pnpm installation
        [System.Environment]::SetEnvironmentVariable('PNPM_HOME', $null, [System.EnvironmentVariableTarget]::Machine)
        [System.Environment]::SetEnvironmentVariable('PNPM_HOME', $null, [System.EnvironmentVariableTarget]::User)
    }

    if (-not $env:CI) {
        Write-Host 'Installing NodeJS...' -ForegroundColor Yellow
        try {
            winget install --exact --no-upgrade --accept-source-agreements --disable-interactivity --id OpenJS.NodeJS
            # Add NodeJS to the PATH
            Add-DirectoryToPath "$env:SystemDrive\Program Files\nodejs"
        } catch {}
    }

   Write-Host 'Installing pnpm...'
   npm install -g "pnpm@latest-$pnpm_major"
   # Add NPM global modules to the PATH
   if (Test-Path "$env:APPDATA\npm" -PathType Container) {
      Add-DirectoryToPath "$env:APPDATA\npm"
   }
}

Write-Host
Write-Host 'Checking for LLVM...' -ForegroundColor Yellow
if ($env:CI) {
    # The CI has LLVM installed already
    Write-Host 'Running with CI, skipping LLVM install.' -ForegroundColor Green
} elseif (
   (Get-Command clang -ea 0) -and (
      (clang --version | Select-String -Pattern 'version\s+(\d+)' | ForEach-Object { $_.Matches.Groups[1].Value }) -eq "$llvm_major"
    )
) {
    Write-Host "LLVM $llvm_major is installed." -ForegroundColor Green
} else {
    $downloadUri = $null
    $releasesUri = 'https://api.github.com/repos/llvm/llvm-project/releases'
    $llvmVersion = "LLVM $llvm_major*"
    $filenamePattern = '*-win64.exe'

    Write-Host "Downloading LLVM $llvm_major installer..." -ForegroundColor Yellow
    $releases = Invoke-RestMethodGithub -Uri $releasesUri
    $downloadUri = $releases | ForEach-Object {
        if ($_.name -like $llvmVersion) {
            $_.assets | Where-Object { $_.name -like $filenamePattern } | Select-Object -ExpandProperty 'browser_download_url'
        }
    } | Select-Object -First 1

    if ($null -eq $downloadUri) {
        Exit-WithError "Couldn't find a LLVM installer for version: $llvm_major"
    }

    $oldUninstaller = "$env:SystemDrive\Program Files\LLVM\Uninstall.exe"
    if (Test-Path $oldUninstaller -PathType Leaf) {
        Exit-WithError 'You have a incompatible version of LLVM installed' 'Uninstall the current version of LLVM and run this script again'
    }

    Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\llvm.exe"

    Write-Host "Installing LLVM $llvm_major" -ForegroundColor Yellow
    Write-Host 'This may take a while and will have no visual feedback, please wait...' -ForegroundColor Cyan
    Start-Process -FilePath "$temp\llvm.exe" -Verb RunAs -ArgumentList '/S' -Wait

    Add-DirectoryToPath "$env:SystemDrive\Program Files\LLVM\bin"

    Remove-Item "$temp\llvm.exe"
}

# Create target folder, continue if already exists
New-Item -ItemType Directory -Path "$projectRoot\target\Frameworks" -Force -ErrorAction SilentlyContinue

$filename = $null
$downloadUri = $null
$releasesUri = 'https://api.github.com/repos/protocolbuffers/protobuf/releases'
$filenamePattern = '*-win64.zip'

Write-Host 'Downloading protobuf compiler...' -ForegroundColor Yellow
$releases = Invoke-RestMethodGithub -Uri $releasesUri
for ($i = 0; $i -lt $releases.Count; $i++) {
    $release = $releases[$i]
    foreach ($asset in $release.assets) {
        if ($asset.name -like $filenamePattern) {
            $filename = $asset.name
            $downloadUri = $asset.browser_download_url
            $i = $releases.Count
            break
        }
    }
}

if (-not ($filename -and $downloadUri)) {
    Exit-WithError "Couldn't find a protobuf compiler installer"
}

Write-Host 'Dowloading protobuf zip...' -ForegroundColor Yellow
Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\protobuf.zip"

Write-Host 'Expanding protobuf zip...' -ForegroundColor Yellow
Expand-Archive "$temp\protobuf.zip" "$projectRoot\target\Frameworks" -Force
Remove-Item "$temp\protobuf.zip"

Write-Host
Write-Host 'Update cargo packages...' -ForegroundColor Yellow
# Run first time to ensure packages are up to date
cargo metadata --format-version 1 > $null

Write-Host
Write-Host 'Retrieving ffmpeg version...' -ForegroundColor Yellow
# Get ffmpeg-sys-next version
$ffmpegVersion = (cargo metadata --format-version 1 | ConvertFrom-Json).packages.dependencies | Where-Object {
    $_.name -like 'ffmpeg-sys-next'
} | Select-Object -ExpandProperty 'req' | ForEach-Object {
    $_ -replace '[~^<>=!*]+', ''
} | Sort-Object -Unique | Select-Object -Last 1

$filename = $null
$downloadUri = $null
$releasesUri = 'https://api.github.com/repos/GyanD/codexffmpeg/releases'
$filenamePattern = '*-full_build-shared.zip'

# Downloads a build of ffmpeg from GitHub compatible with the declared ffmpeg-sys-next version
$releases = Invoke-RestMethodGithub -Uri $releasesUri
$version = $ffmpegVersion
while (-not ($filename -and $downloadUri) -and $version) {
    for ($i = 0; $i -lt $releases.Count; $i++) {
        $release = $releases[$i]
        if ($release.tag_name -eq $version) {
            foreach ($asset in $release.assets) {
                if ($asset.name -like $filenamePattern) {
                    $filename = $asset.name
                    $downloadUri = $asset.browser_download_url
                    $i = $releases.Count
                    break
                }
            }
        }
    }
    $version = $version -replace '\.?\d+$'
}

if (-not ($filename -and $downloadUri)) {
    Exit-WithError "Couldn't find a ffmpeg installer for version: $ffmpegVersion"
}

Write-Host 'Dowloading ffmpeg zip...' -ForegroundColor Yellow
Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\ffmpeg.zip"

Write-Host 'Expanding ffmpeg zip...' -ForegroundColor Yellow
# FFmpeg zip contains a subdirectory with the same name as the zip file
Expand-Archive "$temp\ffmpeg.zip" "$temp" -Force
Remove-Item "$temp\ffmpeg.zip"

$ffmpegDir = "$temp\$([System.IO.Path]::GetFileNameWithoutExtension($fileName))"
robocopy "$ffmpegDir" "$projectRoot\target\Frameworks" /E /NS /NC /NFL /NDL /NP /NJH /NJS
Remove-Item -Path "$ffmpegDir" -Recurse -Force -ErrorAction SilentlyContinue

@(
    '[env]',
    "PROTOC = `"$projectRoot\target\Frameworks\bin\protoc`"",
    "FFMPEG_DIR = `"$projectRoot\target\Frameworks`"",
    '',
    (Get-Content "$projectRoot\.cargo\config.toml" -Raw)
) | Out-File "$projectRoot\.cargo\config" -Encoding utf8

if (-not $env:CI) {
    Write-Host
    Write-Host 'Your machine has been setup for Spacedrive development!' -ForegroundColor Green
    Write-Host 'You will need to re-run this script if there are rust dependencies changes or you use `pnpm clean` or `cargo clean`!' -ForegroundColor Red
    Read-Host 'Press Enter to continue'
}
