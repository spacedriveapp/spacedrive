# Set default value of 0 for external command exit code
$LASTEXITCODE = 0
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

# Reset PATH to ensure the script doesn't have stale Path entries
Reset-Path

# Get project dir (get grandparent dir from script location: <PROJECT_ROOT>\scripts\setup.ps1)
$projectRoot = Split-Path -Path $PSScriptRoot -Parent
$packageJson = Get-Content -Raw -Path "$projectRoot\package.json" | ConvertFrom-Json

# Valid winget exit status
$wingetValidExit = 0, -1978335189, -1978335153, -1978335135

# Currently LLVM >= 16 is not supported due to incompatibilities with ffmpeg-sys-next
# See https://github.com/spacedriveapp/spacedrive/issues/677
$llvmVersion = [Version]'15.0.7'

Write-Host 'Spacedrive Development Environment Setup' -ForegroundColor Magenta
Write-Host @"

To set up your machine for Spacedrive development, this script will do the following:
1) Install Windows C++ build tools
2) Install Edge Webview 2
3) Install Rust and Cargo
4) Install Rust tools
5) Install Strawberry perl (used by to build the openssl-sys crate)
6) Install Node.js, npm and pnpm
7) Install LLVM $llvmVersion (compiler for ffmpeg-sys-next crate)
"@

# Install System dependencies (GitHub Actions already has all of those installed)
if (-not $env:CI) {
    if (-not (Get-Command winget -ea 0)) {
        Exit-WithError 'winget not available' @'
Follow the instructions here to install winget:
https://learn.microsoft.com/windows/package-manager/winget/
'@
    }

    # Check system winget version is greater or equal to v1.4.10052
    $wingetVersion = [Version]((winget --version)  -replace '.*?(\d+)\.(\d+)\.(\d+).*', '$1.$2.$3')
    $requiredVersion = [Version]'1.4.10052'
    if ($wingetVersion.CompareTo($requiredVersion) -lt 0) {
        $errorMessage = "You need to update your winget to version $requiredVersion or higher."
        Exit-WithError $errorMessage
    }

    # Check connectivity to GitHub
    $ProgressPreference = 'SilentlyContinue'
    if (-not ((Test-NetConnection -ComputerName 'github.com' -Port 80).TcpTestSucceeded)) {
        Exit-WithError "Can't connect to github, check your internet connection and run this script again"
    }
    $ProgressPreference = 'Continue'

    Write-Host
    Read-Host 'Press Enter to continue'

    # TODO: Force update Visual Studio build tools
    Write-Host
    Write-Host 'Installing Visual Studio Build Tools...' -ForegroundColor Yellow
    Write-Host 'This will take some time as it involves downloading several gigabytes of data....' -ForegroundColor Cyan
    winget install -e --accept-source-agreements --force --disable-interactivity --id Microsoft.VisualStudio.2022.BuildTools `
        --override 'updateall --quiet --wait'
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
    Write-Host 'Installing Strawberry perl...' -ForegroundColor Yellow
    winget install -e --accept-source-agreements --disable-interactivity --id StrawberryPerl.StrawberryPerl
    if (-not ($wingetValidExit -contains $LASTEXITCODE)) {
        Exit-WithError 'Failed to install Strawberry perl'
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

if ($LASTEXITCODE -ne 0) {
    Exit-WithError "Something went wrong, exit code: $LASTEXITCODE"
}

if (-not $env:CI) {
    Write-Host
    Write-Host 'Your machine has been setup for Spacedrive development!' -ForegroundColor Green
    Write-Host
    Read-Host 'Press Enter to continue'
}
