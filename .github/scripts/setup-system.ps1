# Check if PowerShell is running as administrator
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
   # Start a new PowerShell process with administrator privileges and set the working directory to the directory where the script is located
   Start-Process -FilePath PowerShell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Definition)`"" -WorkingDirectory "$PSScriptRoot"
   # Exit the current PowerShell process
   Exit
}

# Enables strict mode, which causes PowerShell to treat uninitialized variables, undefined functions, and other common errors as terminating errors.
Set-StrictMode -Version Latest

# Get temp folder
$temp = [System.IO.Path]::GetTempPath()

# Get project dir (get grandparent dir from script location: <PROJECT_ROOT>\.github\scripts)
$projectRoot = Split-Path -Path (Split-Path -Path $PSScriptRoot -Parent) -Parent

# Change CWD to project root
Set-Location $projectRoot

Write-Host 'Spacedrive Development Environment Setup' -ForegroundColor Magenta
Write-Host @'

To set up your machine for Spacedrive development, this script will do the following:

1) Check for Rust and Cargo

2) Install pnpm (if not installed)

3) Install the latest version of Node.js using pnpm

4) Install LLVM (compiler for ffmpeg-rust)

4) Download ffmpeg and set as an environment variable

'@

# Check connectivity to GitHub
$ProgressPreference = 'SilentlyContinue'
if (-not ((Test-NetConnection -ComputerName 'github.com' -Port 80).TcpTestSucceeded)) {
   Write-Host
   Write-Host "Can't connect to github, maybe internet is down?"
   Read-Host 'Press Enter to exit'
   Exit 1
}
$ProgressPreference = 'Continue'

# Install C++ and Rust build tools
if (-not $env:CI) {
   if (-not (Get-Command winget -ea 0)) {
      Write-Host
      Write-Error 'winget not available'
      Write-Host @'
Follow the instructions here to install winget:
https://learn.microsoft.com/windows/package-manager/winget/
'@ -ForegroundColor Yellow
      Read-Host 'Press Enter to exit'
      Exit 1
   }

   Write-Host
   Write-Host 'Installing Visual Studio Build Tools...' -ForegroundColor Yellow
   Write-Host 'This will take a while...'
   Start-Sleep -Milliseconds 150
   # Force install because BuildTools is itself a installer of multiple packages, let it decide if it is already installed or not
   winget install --exact --no-upgrade --accept-source-agreements --force --disable-interactivity --id Microsoft.VisualStudio.2022.BuildTools --override '--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'

   Write-Host
   Write-Host 'Installing Edge Webview 2...' -ForegroundColor Yellow
   Start-Sleep -Milliseconds 150
   try {
      # This is normally already available, but on some early Windows 10 versions it isn't
      winget install --exact --no-upgrade --accept-source-agreements --disable-interactivity --id Microsoft.EdgeWebView2Runtime
   } catch {}

   Write-Host
   Write-Host 'Installing Rust and Cargo...' -ForegroundColor Yellow
   Start-Sleep -Milliseconds 150
   try {
      winget install --exact --no-upgrade --accept-source-agreements --disable-interactivity --id Rustlang.Rustup
   } catch {}

   Write-Host
   Write-Host 'Installing Rust tools' -ForegroundColor Yellow
   Start-Sleep -Milliseconds 150
   cargo install cargo-watch
}

Write-Host
Write-Host 'Checking for pnpm...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

$pnpm_major = '7'
if ((Get-Command pnpm -ea 0) -and (pnpm --version | Select-Object -First 1) -match "^$pnpm_major\." ) {
   Write-Host "pnpm $pnpm_major is installed." -ForegroundColor Green
} else {
   Write-Host "pnpm $pnpm_major is not installed. Installing now."
   Write-Host 'Running the pnpm installer...'

   # Currently pnpm >= 8 is not supported due to incompatibilities with some dependencies
   $env:PNPM_VERSION = "latest-$pnpm_major"

   # pnpm installer taken from https://pnpm.io
   Invoke-WebRequest https://get.pnpm.io/install.ps1 -useb | Invoke-Expression

   # Reset the PATH env variables to make sure pnpm is accessible 
   $env:PNPM_HOME = [System.Environment]::GetEnvironmentVariable('PNPM_HOME', 'User')
   $env:Path = [System.Environment]::ExpandEnvironmentVariables([System.Environment]::GetEnvironmentVariable('Path', 'User'))
}

Write-Host
Write-Host 'Checking for node...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

# A GitHub Action takes care of installing node, so this isn't necessary if running in the ci.
if ($env:CI) {
   Write-Host 'Running with Ci, skipping Node install.' -ForegroundColor Green
} else {
   Write-Host 'Using pnpm to install the latest version of Node...' -ForegroundColor Yellow
   Write-Host 'This will set your global Node version to the latest!'
   Start-Sleep -Milliseconds 150

   # Runs the pnpm command to use the latest version of node, which also installs it
   Start-Process -Wait -FilePath 'pnpm' -ArgumentList 'env use --global latest' -PassThru -Verb runAs

   # Workaround issues
   # https://github.com/pnpm/pnpm/issues/5266
   # https://github.com/pnpm/pnpm/issues/5700
   if (Test-Path "$env:PNPM_HOME\pnpm.exe" -PathType Leaf) {
      try { pnpm add -g pnpm@"latest-$pnpm_major" 2>&1 | Out-Null } catch {}
      Remove-Item "$env:PNPM_HOME\pnpm.exe"
      $pnpm = (Get-ChildItem $env:PNPM_HOME -Recurse -File -Filter pnpm.js | Select-Object -First 1).fullname
      node $pnpm add -g pnpm@"latest-$pnpm_major"
   }
}

Write-Host
Write-Host 'Checking for LLVM...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

$llvm_major = '15'
if ($env:CI) {
   # The ci has LLVM installed already, so we instead just set the env variables.
   Write-Host 'Running with Ci, skipping LLVM install.' -ForegroundColor Green

   # TODO: Check if CI LLVM version match our required major version
   $VCINSTALLDIR = $(& "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -latest -property installationPath)
   Add-Content $env:GITHUB_ENV "LIBCLANG_PATH=${VCINSTALLDIR}\VC\Tools\LLVM\x64\bin`n"
} elseif (
   (Get-Command clang -ea 0) -and (
      (clang --version | Select-String -Pattern 'version\s+(\d+)' | ForEach-Object { $_.Matches.Groups[1].Value }) -eq "$llvm_major"
   )
) {
   Write-Host "LLVM $llvm_major is installed." -ForegroundColor Green
} else {
   Write-Host
   Write-Host "Downloading the LLVM $llvm_major installer..." -ForegroundColor Yellow

   # Downloads latest installer for LLVM
   $releasesUri = 'https://api.github.com/repos/llvm/llvm-project/releases'
   $llvmVersion = "LLVM $llvm_major*"
   $filenamePattern = '*-win64.exe'
   $releases = Invoke-RestMethod -Uri $releasesUri
   $downloadUri = $releases | ForEach-Object {
      if ($_.name -like $llvmVersion) {
         $_.assets | Where-Object { $_.name -like $filenamePattern } | Select-Object -ExpandProperty 'browser_download_url'
      }
   } | Select-Object -First 1

   if ($null -eq $downloadUri) {
      Write-Error "Error: Couldn't find a LLVM installer for version: $llvm_major"
      Read-Host 'Press Enter to exit'
      Exit 1
   }

   Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\llvm.exe"

   Write-Host
   Write-Host 'Running the LLVM installer...' -ForegroundColor Yellow
   Write-Host @'
Please follow the instructions to install LLVM.
Uninstall any previous versions of LLVM, if necessary.
Ensure you add LLVM to your PATH.
'@ -ForegroundColor Red

   Start-Process -Wait -FilePath "$temp\llvm.exe" -PassThru -Verb runAs
}

Write-Host
Write-Host 'Install protobuf compiler...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

$protocVersion = $null
if (($null -ne $env:PROTOC) -and (Test-Path $env:PROTOC -PathType Leaf)) {
   $protocVersion = &"$env:PROTOC" --version 2>&1 | Out-String
}

if ($protocVersion) {
   Write-Host 'protobuf compiler is installed.' -ForegroundColor Green
} else {
   $filename = $null
   $downloadUri = $null
   $releasesUri = 'https://api.github.com/repos/protocolbuffers/protobuf/releases'
   $filenamePattern = '*-win64.zip'

   # Downloads a build of protobuf from GitHub
   $releases = Invoke-RestMethod -Uri $releasesUri
   # Downloads a build of protobuf from GitHub compatible with the declared protobuf version
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
      Write-Error "Error: Couldn't find a protobuf compiler installer"
      Read-Host 'Press Enter to exit'
      Exit 1
   }

   $foldername = "$env:LOCALAPPDATA\$([System.IO.Path]::GetFileNameWithoutExtension($fileName))"
   New-Item -Path $foldername -ItemType Directory -ErrorAction SilentlyContinue

   Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\protobuf.zip"

   Write-Host
   Write-Host 'Expanding protobuf zip...' -ForegroundColor Yellow

   Expand-Archive "$temp\protobuf.zip" $foldername -ErrorAction SilentlyContinue
   Remove-Item "$temp\protobuf.zip"

   Write-Host
   Write-Host 'Setting environment variables...' -ForegroundColor Yellow

   # Sets environment variable for protobuf
   [System.Environment]::SetEnvironmentVariable('PROTOC', "$foldername\bin\protoc.exe", [System.EnvironmentVariableTarget]::User)

   if ($env:CI) {
      # If running in ci, we need to use GITHUB_ENV and GITHUB_PATH instead of the normal PATH env variables, so we set them here
      Add-Content $env:GITHUB_ENV "PROTOC=$foldername\bin\protoc.exe`n"
      Add-Content $env:GITHUB_PATH "$foldername\bin`n"
   }
}

Write-Host
Write-Host 'Update cargo packages...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

# Run first time to ensure packages are up to date
cargo metadata --format-version 1 > $null

Write-Host
Write-Host 'Downloading the latest ffmpeg build...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

# Get ffmpeg-sys-next version
$ffmpegVersion = (cargo metadata --format-version 1 | ConvertFrom-Json).packages.dependencies | Where-Object {
   $_.name -like 'ffmpeg-sys-next'
} | Select-Object -ExpandProperty 'req' | ForEach-Object {
   $_ -replace '[~^<>=!*]+', ''
} | Sort-Object -Unique

if (($null -ne $env:FFMPEG_DIR) -and (
      $ffmpegVersion.StartsWith(
         (($env:FFMPEG_DIR.split('\') | Where-Object { $_ -like 'ffmpeg-*' }) -replace 'ffmpeg-(\d+(\.\d+)*).*', '$1'),
         [System.StringComparison]::InvariantCulture
      )
   )
) {
   Write-Host 'ffmpeg is installed.' -ForegroundColor Green
} else {
   $filename = $null
   $downloadUri = $null
   $releasesUri = 'https://api.github.com/repos/GyanD/codexffmpeg/releases'
   $filenamePattern = '*-full_build-shared.zip'

   # Downloads a build of ffmpeg from GitHub compatible with the declared ffmpeg-sys-next version
   $releases = Invoke-RestMethod -Uri $releasesUri
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
      Write-Error "Error: Couldn't find a ffmpeg installer for version: $ffmpegVersion"
      Read-Host 'Press Enter to exit'
      Exit 1
   }

   $foldername = "$env:LOCALAPPDATA\$([System.IO.Path]::GetFileNameWithoutExtension($fileName))"

   Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\ffmpeg.zip"

   Write-Host
   Write-Host 'Expanding ffmpeg zip...' -ForegroundColor Yellow

   # FFmpeg zip contains a subdirectory with the same name as the zip file
   Expand-Archive "$temp\ffmpeg.zip" $env:LOCALAPPDATA -ErrorAction SilentlyContinue
   Remove-Item "$temp\ffmpeg.zip"

   Write-Host
   Write-Host 'Setting environment variables...' -ForegroundColor Yellow

   # Sets environment variable for ffmpeg
   [System.Environment]::SetEnvironmentVariable('FFMPEG_DIR', "$foldername", [System.EnvironmentVariableTarget]::User)
   $env:FFMPEG_DIR = "$foldername"

   if ($env:CI) {
      # If running in ci, we need to use GITHUB_ENV and GITHUB_PATH instead of the normal PATH env variables, so we set them here
      Add-Content $env:GITHUB_ENV "FFMPEG_DIR=$foldername`n"
      Add-Content $env:GITHUB_PATH "$foldername\bin`n"
   }
}

Write-Host
Write-Host 'Copying Required .dll files...' -ForegroundColor Yellow
Start-Sleep -Milliseconds 150

# Create target\debug folder, continue if already exists
New-Item -Path $projectRoot\target\debug -ItemType Directory -ErrorAction SilentlyContinue

# Copies all .dll required for rust-ffmpeg to target\debug folder
Get-ChildItem "$env:FFMPEG_DIR\bin" -Recurse -Filter *.dll | Copy-Item -Destination "$projectRoot\target\debug"

Write-Host
Write-Host 'Your machine has been setup for Spacedrive development!' -ForegroundColor Green
Write-Host 'You will need to re-run this script if you use `pnpm clean` or `cargo clean`' -ForegroundColor Red
Read-Host 'Press Enter to exit'
