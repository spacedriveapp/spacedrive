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

$ghUrl = 'https://api.github.com/repos'
$sdGhPath = 'spacedriveapp/spacedrive'

function Invoke-RestMethodGithub {
    [CmdletBinding()]
    param (
        [Parameter(Mandatory = $true)]
        [string]$Uri,
        [string]$Method = 'GET',
        [string]$OutFile = $null,
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
        OutFile   = $OutFile
        Headers   = $Headers
        UserAgent = $UserAgent
    }

    Invoke-RestMethod @params
}

function DownloadArtifact {
    param (
        [Parameter(Mandatory = $true)]
        [ValidateNotNullOrEmpty()]
        [string]$ArtifactPath,
        [string]$OutFile
    )

    try {
        Invoke-RestMethodGithub -Uri "$ghUrl/$sdGhPath/actions/artifacts/$($($ArtifactPath -split '/')[3])/zip" -OutFile $OutFile
    } catch {
        # nightly.link is a workaround for the lack of a public GitHub API to download artifacts from a workflow run
        # https://github.com/actions/upload-artifact/issues/51
        # Use it when running in environments that are not authenticated with GitHub
        Write-Host 'Failed to download artifact from Github, falling back to nightly.link' -ForegroundColor Yellow
        Invoke-RestMethodGithub -Uri "https://nightly.link/${sdGhPath}/${ArtifactPath}" -OutFile $OutFile
    }
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

$ffmpegVersion = '6.0'

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

    # TODO: Check system winget version is greater or equal to v1.4.10052

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

    # TODO: Install Strawberry perl, required by debug build of openssl-sys

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
Write-Host 'Retrieving protobuf build...' -ForegroundColor Yellow

$filename = $null
$downloadUri = $null
$releasesUri = "${ghUrl}/protocolbuffers/protobuf/releases"
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

Write-Host "Dowloading protobuf zip from ${downloadUri}..." -ForegroundColor Yellow
Invoke-RestMethodGithub -Uri $downloadUri -OutFile "$temp\protobuf.zip"

Write-Host 'Expanding protobuf zip...' -ForegroundColor Yellow
Expand-Archive "$temp\protobuf.zip" "$projectRoot\target\Frameworks" -Force
Remove-Item -Force -ErrorAction SilentlyContinue -Path "$temp\protobuf.zip"

# --

Write-Host "Retrieving ffmpeg-${ffmpegVersion} build..." -ForegroundColor Yellow

$page = 1
while ($page -gt 0) {
    $success = ''
    Invoke-RestMethodGithub -Uri `
        "${ghUrl}/${sdGhPath}/actions/workflows/ffmpeg-windows.yml/runs?page=$page&per_page=100&status=success" `
    | ForEach-Object {
        if (-not $_.workflow_runs) {
            Exit-WithError "Error: $_"
        }

        $_.workflow_runs | ForEach-Object {
            $artifactPath = (
                (Invoke-RestMethodGithub -Uri ($_.artifacts_url | Out-String) -Method Get).artifacts `
                | Where-Object {
                    $_.name -eq "ffmpeg-${ffmpegVersion}-x86_64"
                } | ForEach-Object {
                    $id = $_.id
                    $workflowRunId = $_.workflow_run.id
                    "suites/${workflowRunId}/artifacts/${id}"
                } | Select-Object -First 1
            )

            try {
                if ([string]::IsNullOrEmpty($artifactPath)) {
                    throw 'Empty argument'
                }

                # Download and extract the artifact
                Write-Host "Dowloading ffmpeg-${ffmpegVersion} zip from artifact ${artifactPath}..." -ForegroundColor Yellow

                DownloadArtifact -ArtifactPath $artifactPath -OutFile "$temp/ffmpeg.zip"

                Write-Host "Expanding ffmpeg-${ffmpegVersion} zip..." -ForegroundColor Yellow
                Expand-Archive "$temp/ffmpeg.zip" "$projectRoot\target\Frameworks" -Force
                Remove-Item -Force -ErrorAction SilentlyContinue -Path "$temp/ffmpeg.zip"

                $success = 'yes'
                break
            } catch {
                $errorMessage = $_.Exception.Message
                Write-Host "Error: $errorMessage" -ForegroundColor Red
                Write-Host 'Failed to download ffmpeg artifact release, trying again in 1sec...'
                Start-Sleep -Seconds 1
                continue
            }
        }
    }

    if ($success -eq 'yes') {
        break
    }

    $page++
    Write-Output 'ffmpeg artifact not found, trying again in 1sec...'
    Start-Sleep -Seconds 1
}

if ($success -ne 'yes') {
    Exit-WithError 'Failed to download ffmpeg files'
}

@(
    '[env]',
    "PROTOC = `"$("$projectRoot\target\Frameworks\bin\protoc" -replace '\\', '\\')`"",
    "FFMPEG_DIR = `"$("$projectRoot\target\Frameworks" -replace '\\', '\\')`"",
    '',
    '[target.x86_64-pc-windows-msvc]',
    "rustflags = [`"-L`", `"$("$projectRoot\target\Frameworks\lib" -replace '\\', '\\')`"]",
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
