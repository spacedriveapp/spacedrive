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
    $proc = Start-Process -PassThru -Wait -FilePath 'PowerShell.exe' -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Definition)`"" -WorkingDirectory "$PSScriptRoot"
    # Reset path so the user doesn't have to restart the shell to use the tools installed by this script
    Reset-Path
    Exit $proc.ExitCode
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
    Reset-Path
    if ($env:Path.Split([IO.Path]::PathSeparator) -notcontains $directory) {
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
$packageJson = Get-Content -Raw -Path "$projectRoot\package.json" | ConvertFrom-Json

# Valid winget exit status
$wingetValidExit = 0, -1978335189, -1978335153, -1978335135

# Currently LLVM >= 16 is not supported due to incompatibilities with ffmpeg-sys-next
# See https://github.com/spacedriveapp/spacedrive/issues/677
$llvmVersion = [Version]'15.0.7'

# Change CWD to project root
Set-Location $projectRoot
Remove-Item -Force -ErrorAction SilentlyContinue -Path "$projectRoot\.cargo\config"
Remove-Item -Force -ErrorAction SilentlyContinue -Path "$projectRoot\target\Frameworks" -Recurse

Write-Host 'Spacedrive Development Environment Setup' -ForegroundColor Magenta
Write-Host @"

To set up your machine for Spacedrive development, this script will do the following:
1) Install Windows C++ build tools
2) Install Edge Webview 2
3) Install Rust and Cargo
4) Install Rust tools
5) Install Node.js, npm and pnpm
6) Install LLVM $llvmVersion (compiler for ffmpeg-rust)
7) Download the protbuf compiler
8) Download a compatible ffmpeg build
"@

# Install System dependencies (GitHub Actions already has all of those installed)
if (-not $env:CI) {
    if (-not (Get-Command winget -ea 0)) {
        Exit-WithError 'winget not available' @'
Follow the instructions here to install winget:
https://learn.microsoft.com/windows/package-manager/winget/
'@
    }

    # Check connectivity to GitHub
    $ProgressPreference = 'SilentlyContinue'
    if (-not ((Test-NetConnection -ComputerName 'github.com' -Port 80).TcpTestSucceeded)) {
        Exit-WithError "Can't connect to github, check your internet connection and run this script again"
    }
    $ProgressPreference = 'Continue'

    Write-Host
    Read-Host 'Press Enter to continue'

    Write-Host
    Write-Host 'Installing Visual Studio Build Tools...' -ForegroundColor Yellow
    Write-Host 'This will take some time as it involves downloading several gigabytes of data....' -ForegroundColor Cyan
    # Force install because BuildTools is itself a package manager, so let it decide if something needs to be installed or not
    winget install -e --accept-source-agreements --force --disable-interactivity --id Microsoft.VisualStudio.2022.BuildTools `
        --override '--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Visual Studio Build Tools'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing Edge Webview 2...' -ForegroundColor Yellow
    # This is normally already available, but on some early Windows 10 versions it isn't
    winget install -e --accept-source-agreements --disable-interactivity --id Microsoft.EdgeWebView2Runtime
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Edge Webview 2'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing Rust and Cargo...' -ForegroundColor Yellow
    winget install -e --accept-source-agreements --disable-interactivity --id Rustlang.Rustup
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Rust and Cargo'
    } else {
        $LASTEXITCODE = 0
    }

    Write-Host
    Write-Host 'Installing NodeJS...' -ForegroundColor Yellow
    # Check if Node.JS is already installed and if it's compatible with the project
    $currentNode = Get-Command node -ea 0
    $currentNodeVersion = if (-not $currentNode) { $null } elseif ($currentNode.Version) { $currentNode.Version } elseif ((node --version) -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    $enginesNodeVersion = if ($packageJson.engines.node -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    if ($currentNodeVersion -and $enginesNodeVersion -and $currentNodeVersion.CompareTo($enginesNodeVersion) -lt 0) {
        Exit-WithError "Current Node.JS version: $currentNodeVersion (required: $enginesNodeVersion)" `
            'Uninstall the current version of Node.JS and run this script again'
    }
    # Install Node.JS
    winget install -e --accept-source-agreements --disable-interactivity --id OpenJS.NodeJS
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install NodeJS'
    } else {
        $LASTEXITCODE = 0
    }
    # Add NodeJS to the PATH
    Add-DirectoryToPath "$env:SystemDrive\Program Files\nodejs"

    Write-Host
    Write-Host 'Checking for LLVM...' -ForegroundColor Yellow
    # Check if LLVM is already installed and if it's compatible with the project
    $currentLLVMVersion = if ("$(winget list -e --disable-interactivity --id LLVM.LLVM)" -match '(?sm)LLVM.LLVM\s+(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    if ($currentLLVMVersion -and $currentLLVMVersion.Major -gt $llvmVersion.Major) {
        Exit-WithError "Current LLVM version: $currentLLVMVersion (required: $llvmVersion)" `
            'Uninstall the current version of LLVM and run this script again'
    }
    # Install LLVM
    winget install -e --accept-source-agreements --disable-interactivity --id LLVM.LLVM --version "$llvmVersion"
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install NodeJS'
    } else {
        $LASTEXITCODE = 0
    }
    # Add LLVM to the PATH
    Add-DirectoryToPath "$env:SystemDrive\Program Files\LLVM\bin"

    # Reset Path to ensure that executable installed above are available to rest of the script
    Reset-Path

    Write-Host
    Write-Host 'Installing Rust MSVC Toolchain...' -ForegroundColor Yellow
    rustup toolchain install stable-msvc
    if ($LASTEXITCODE -ne 0) {
        Exit-WithError 'Failed to install Rust MSVC Toolchain'
    }

    Write-Host
    Write-Host 'Installing Rust tools...' -ForegroundColor Yellow
    cargo install cargo-watch
    if ($LASTEXITCODE -ne 0) {
        Exit-WithError 'Failed to install Rust tools'
    }

    Write-Host
    Write-Host 'Installing for pnpm...' -ForegroundColor Yellow
    # Check if pnpm is already installed and if it's compatible with the project
    $currentPnpmVersion = if (-not (Get-Command pnpm -ea 0)) { $null } elseif ((pnpm --version) -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }
    $enginesPnpmVersion = if ($packageJson.engines.pnpm -match '(?sm)(\d+(\.\d+)*)') { [Version]$matches[1] } else { $null }

    if (-not $currentPnpmVersion) {
        # Remove possible remaining envvars from old pnpm installation
        [System.Environment]::SetEnvironmentVariable('PNPM_HOME', $null, [System.EnvironmentVariableTarget]::Machine)
        [System.Environment]::SetEnvironmentVariable('PNPM_HOME', $null, [System.EnvironmentVariableTarget]::User)

        # Install pnpm
        npm install -g "pnpm@latest-$($enginesPnpmVersion.Major)"
        if ($LASTEXITCODE -ne 0) {
            Exit-WithError 'Failed to install pnpm'
        }

        # Add NPM global modules to the PATH
        if (Test-Path "$env:APPDATA\npm" -PathType Container) {
            Add-DirectoryToPath "$env:APPDATA\npm"
        }
    } elseif ($currentPnpmVersion -and $enginesPnpmVersion -and $currentPnpmVersion.CompareTo($enginesPnpmVersion) -lt 0) {
        Exit-WithError "Current pnpm version: $currentPnpmVersion (required: $enginesPnpmVersion)" `
            'Uninstall the current version of pnpm and run this script again'
    }
}

# Create target folder, continue if already exists
New-Item -Force -ErrorAction SilentlyContinue -ItemType Directory -Path "$projectRoot\target\Frameworks" | Out-Null

# --

Write-Host
Write-Host 'Retrieving protobuf version...' -ForegroundColor Yellow

$filename = $null
$downloadUri = $null
$releasesUri = 'https://api.github.com/repos/protocolbuffers/protobuf/releases'
$filenamePattern = '*-win64.zip'

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
Remove-Item -Force -ErrorAction SilentlyContinue -Path "$temp\protobuf.zip"

# --

Write-Host
Write-Host 'Retrieving ffmpeg version...' -ForegroundColor Yellow

# Run first to update packages
cargo metadata --format-version 1 | Out-Null

# Get ffmpeg-sys-next version
$ffmpegVersion = (cargo metadata --format-version 1 | ConvertFrom-Json).packages.dependencies | Where-Object {
    $_.name -like 'ffmpeg-sys-next'
} | Select-Object -ExpandProperty 'req' | ForEach-Object {
    $_ -replace '[~^<>=!*]+', ''
} | Sort-Object -Unique | Select-Object -Last 1

if ($LASTEXITCODE -ne 0) {
    Exit-WithError 'Failed to get ffmpeg-sys-next version'
}

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
Expand-Archive -Force -Path "$temp\ffmpeg.zip" -DestinationPath "$temp"
Remove-Item -Force -ErrorAction SilentlyContinue -Path "$temp\ffmpeg.zip"

$ffmpegDir = "$temp\$([System.IO.Path]::GetFileNameWithoutExtension($fileName))"
$proc = Start-Process -PassThru -Wait -FilePath 'Robocopy.exe' -Verb RunAs -ArgumentList `
    "`"$ffmpegDir`" `"$projectRoot\target\Frameworks`" /E /NS /NC /NFL /NDL /NP /NJH /NJS"
# https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/robocopy#exit-return-codes
if ($proc.ExitCode -ge 8) {
    Exit-WithError 'Failed to copy ffmpeg files'
}
Remove-Item -Force -ErrorAction SilentlyContinue -Recurse -Path "$ffmpegDir"

@(
    '[env]',
    "PROTOC = `"$("$projectRoot\target\Frameworks\bin\protoc" -replace '\\', '\\')`"",
    "FFMPEG_DIR = `"$("$projectRoot\target\Frameworks" -replace '\\', '\\')`"",
    '',
    (Get-Content "$projectRoot\.cargo\config.toml" -Encoding utf8)
) | Out-File -Force -Encoding utf8 -FilePath "$projectRoot\.cargo\config"

if (-not $env:CI) {
    Write-Host
    Write-Host 'Your machine has been setup for Spacedrive development!' -ForegroundColor Green
    Write-Host 'You will need to re-run this script if there are rust dependencies changes or you use `pnpm clean` or `cargo clean`!' -ForegroundColor Red
    Write-Host
    Read-Host 'Press Enter to continue'
}

if ($LASTEXITCODE -ne 0) {
    Exit-WithError "Something went wrong, exit code: $LASTEXITCODE"
}
