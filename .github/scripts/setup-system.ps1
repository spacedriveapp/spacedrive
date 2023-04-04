# Check if PowerShell is running as administrator
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
   # Start a new PowerShell process with administrator privileges
   Start-Process -FilePath PowerShell.exe -Verb RunAs -ArgumentList $MyInvocation.MyCommand.Definition
   # Exit the current PowerShell process
   Exit
}

# Get temp folder
$temp = [System.IO.Path]::GetTempPath()

# Get current running dir
$currentLocation = $((Get-Location).path)

Write-Host 'Spacedrive Development Environment Setup' -ForegroundColor Magenta
Write-Host @'

To set up your machine for Spacedrive development, this script will do the following:

1) Check for Rust and Cargo

2) Install pnpm (if not installed)

3) Install the latest version of Node.js using pnpm

4) Install LLVM (compiler for ffmpeg-rust)

4) Download ffmpeg and set as an environment variable

'@

Start-Sleep -Milliseconds 150

Write-Host 'Checking for Rust and Cargo...' -ForegroundColor Yellow
if (!(Get-Command cargo -ea 0)) {
   Write-Host @'
Cargo is not installed.

To use Spacedrive on Windows, Cargo needs to be installed.
The Visual Studio C++ Build tools are also required.
Instructions can be found here:

https://tauri.app/v1/guides/getting-started/prerequisites/#setting-up-windows

Once you have installed Cargo, re-run this script.

'@
   Exit
} else {
   Write-Host 'Cargo is installed.'
}

if ($env:CI -ne $True) {
   Write-Host 'Installing Rust tools' -ForegroundColor Yellow
   cargo install cargo-watch
}

Write-Host
Write-Host 'Checking for pnpm...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

if (!(Get-Command pnpm -ea 0)) {
   Write-Host 'pnpm is not installed. Installing now.'
   Write-Host 'Running the pnpm installer...'

   # Currently pnpm >= 8 is not supported due to incompatbilities with some dependencies
   $env:PNPM_VERSION = 'latest-7'

   #pnpm installer taken from https://pnpm.io
   Invoke-WebRequest https://get.pnpm.io/install.ps1 -useb | Invoke-Expression

   # Reset the PATH env variables to make sure pnpm is accessible 
   $env:PNPM_HOME = [System.Environment]::GetEnvironmentVariable('PNPM_HOME', 'User')
   $env:Path = [System.Environment]::ExpandEnvironmentVariables([System.Environment]::GetEnvironmentVariable('Path', 'User'))
} else {
   Write-Host 'pnpm is installed.'
}

# A GitHub Action takes care of installing node, so this isn't necessary if running in the ci.
if ($env:CI -eq $True) {
   Write-Host
   Write-Host 'Running with Ci, skipping Node install.' -ForegroundColor Yellow
} else {
   Write-Host
   Write-Host 'Using pnpm to install the latest version of Node...' -ForegroundColor Yellow
   Write-Host 'This will set your global Node version to the latest!'
   Start-Sleep -Milliseconds 150

   # Runs the pnpm command to use the latest version of node, which also installs it
   Start-Process -Wait -FilePath 'pnpm' -ArgumentList 'env use --global latest' -PassThru -Verb runAs
}

# The ci has LLVM installed already, so we instead just set the env variables.
if ($env:CI -eq $True) {
   Write-Host
   Write-Host 'Running with Ci, skipping LLVM install.' -ForegroundColor Yellow

   $VCINSTALLDIR = $(& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -property installationPath)
   Add-Content $env:GITHUB_ENV "LIBCLANG_PATH=${VCINSTALLDIR}\VC\Tools\LLVM\x64\bin`n"
} elseif (
   !(Get-Command clang -ea 0) -or (
      (clang --version | Select-String -Pattern 'version\s+(\d+)' | ForEach-Object { $_.Matches.Groups[1].Value }) -ne 15
   )
) {
   Write-Host
   Write-Host 'Downloading the LLVM 15 installer...' -ForegroundColor Yellow

   # Downloads latest installer for LLVM
   $releasesUri = 'https://api.github.com/repos/llvm/llvm-project/releases'
   $versionPattern = 'LLVM 15*'
   $filenamePattern = '*-win64.exe'
   $releases = Invoke-RestMethod -Uri $releasesUri
   $downloadUri = $releases | ForEach-Object {
      if ($_.name -like $versionPattern) {
         $_.assets | Where-Object { $_.name -like $filenamePattern } | Select-Object -ExpandProperty 'browser_download_url'
      }
   } | Select-Object -First 1

   Start-BitsTransfer -Source $downloadUri -Destination "$temp\llvm.exe"

   Write-Host
   Write-Host 'Running the LLVM installer...' -ForegroundColor Yellow
   Write-Host 'Please follow the instructions to install LLVM.'
   Write-Host 'Uninstall any previous versions of LLVM, if necessary.'
   Write-Host 'Ensure you add LLVM to your PATH.'

   Start-Process "$temp\llvm.exe" -Wait
} else {
   Write-Host
   Write-Host 'LLVM is installed.'
}

# Install chocolatey if it isn't already installed
if (!(Get-Command choco -ea 0)) {
   Write-Host
   Write-Host 'Installing Chocolatey...' -ForegroundColor Yellow
   Set-ExecutionPolicy Bypass -Scope Process -Force
   [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
   Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
} else {
   Write-Host
   Write-Host 'Chocolatey is installed.'
}

Write-Host
Write-Host 'Install protobuf compiler...' -ForegroundColor Yellow
choco install -y protoc

Write-Host
Write-Host 'Downloading the latest ffmpeg build...' -ForegroundColor Yellow

# Downloads the latest shared build of ffmpeg from GitHub
# $filenamePattern = "*-full_build-shared.zip"
# $releasesUri = "https://api.github.com/repos/GyanD/codexffmpeg/releases/latest"
$ffmpegVersion = '5.1.1'
$downloadUri = "https://github.com/GyanD/codexffmpeg/releases/download/$ffmpegVersion/ffmpeg-$ffmpegVersion-full_build-shared.zip" # ((Invoke-RestMethod -Method GET -Uri $releasesUri).assets | Where-Object name -like $filenamePattern ).browser_download_url
$filename = "ffmpeg-$ffmpegVersion-full_build-shared.zip" # ((Invoke-RestMethod -Method GET -Uri $releasesUri).assets | Where-Object name -like $filenamePattern ).name
$remove = '.zip'
$foldername = $filename.Substring(0, ($filename.Length - $remove.Length))

Start-BitsTransfer -Source $downloadUri -Destination "$temp\ffmpeg.zip"

Write-Host
Write-Host 'Expanding ffmpeg zip...' -ForegroundColor Yellow

Expand-Archive "$temp\ffmpeg.zip" $HOME -ErrorAction SilentlyContinue

Remove-Item "$temp\ffmpeg.zip"

Write-Host
Write-Host 'Setting environment variables...' -ForegroundColor Yellow

if ($env:CI -eq $True) {
   # If running in ci, we need to use GITHUB_ENV and GITHUB_PATH instead of the normal PATH env variables, so we set them here
   Add-Content $env:GITHUB_ENV "FFMPEG_DIR=$HOME\$foldername`n"
   Add-Content $env:GITHUB_PATH "$HOME\$foldername\bin`n" 
} else {
   # Sets environment variable for ffmpeg
   [System.Environment]::SetEnvironmentVariable('FFMPEG_DIR', "$HOME\$foldername", [System.EnvironmentVariableTarget]::User)
}

Write-Host
Write-Host 'Copying Required .dll files...' -ForegroundColor Yellow

# Create target\debug folder, continue if already exists
New-Item -Path $currentLocation\target\debug -ItemType Directory -ErrorAction SilentlyContinue

# Copies all .dll required for rust-ffmpeg to target\debug folder
Get-ChildItem "$HOME\$foldername\bin" -Recurse -Filter *.dll | Copy-Item -Destination "$currentLocation\target\debug"

Write-Host
Write-Host 'Your machine has been setup for Spacedrive development!'
Write-Host -NoNewline 'Press any key to continue...'
$Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')
