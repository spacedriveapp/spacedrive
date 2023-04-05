# Check if PowerShell is running as administrator
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
   # Start a new PowerShell process with administrator privileges and set the working directory to the directory where the script is located
   Start-Process -FilePath PowerShell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Definition)`"" -WorkingDirectory "$PSScriptRoot"
   # Exit the current PowerShell process
   Exit
}

function Add-DirectoryToPath($directory) {
   if ($env:Path.Split(';') -notcontains $directory) {
      [Environment]::SetEnvironmentVariable('Path', "$($env:Path);$directory", [System.EnvironmentVariableTarget]::User)
      $env:Path = [Environment]::GetEnvironmentVariable('Path', [System.EnvironmentVariableTarget]::User)
      [System.Environment]::SetEnvironmentVariable('Path', $env:Path, [System.EnvironmentVariableTarget]::Process)
   }
}

function Wait-UserInput() {
   if (-not $env:CI) { Read-Host 'Press Enter to continue' }
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
1) Install Windows C++ build tools
2) Install Edge Webview 2
3) Install Rust and Cargo
4) Install Node.js
5) Install Rust tools
6) Install pnpm
7) Install LLVM (compiler for ffmpeg-rust)
8) Install protbuf compiler and set as an environment variable
9) Download ffmpeg and set as an environment variable
'@

# Check connectivity to GitHub
$ProgressPreference = 'SilentlyContinue'
if (-not ((Test-NetConnection -ComputerName 'github.com' -Port 80).TcpTestSucceeded)) {
   Write-Host
   Write-Host "Can't connect to github, maybe internet is down?"
   Write-Host
   Wait-UserInput
   Exit 1
}
$ProgressPreference = 'Continue'

# Install C++ build tools, Rust and NodeJS
# GitHub Actions already has all of this installed
if (-not $env:CI) {
   if (-not (Get-Command winget -ea 0)) {
      Write-Host
      Write-Error 'winget not available'
      Write-Host @'
Follow the instructions here to install winget:
https://learn.microsoft.com/windows/package-manager/winget/
'@ -ForegroundColor Yellow
      Wait-UserInput
      Exit 1
   }

   Write-Host
   Write-Host 'Installing Visual Studio Build Tools...' -ForegroundColor Yellow
   Write-Host 'This will take a while...'
   # Force install because BuildTools is itself a installer of multiple packages, let it decide if it is already installed or not
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
      # Reset Path to ensure cargo is available for the rest of the script
      $env:Path = [System.Environment]::ExpandEnvironmentVariables([System.Environment]::GetEnvironmentVariable('Path', [System.EnvironmentVariableTarget]::User))
   } catch {}

   Write-Host
   Write-Host 'Installing NodeJS...' -ForegroundColor Yellow
   try {
      winget install --exact --no-upgrade --accept-source-agreements --disable-interactivity --id OpenJS.NodeJS
      # Add NodeJS to the PATH
      Add-DirectoryToPath "$env:SystemDrive\Program Files\nodejs"
   } catch {}

   Write-Host
   Write-Host 'Installing Rust tools' -ForegroundColor Yellow
   cargo install cargo-watch
}

Write-Host
Write-Host 'Checking for pnpm...' -ForegroundColor Yellow
$pnpm_major = '7' # Currently pnpm >= 8 is not supported due to incompatibilities with some dependencies
if ((Get-Command pnpm -ea 0) -and (pnpm --version | Select-Object -First 1) -match "^$pnpm_major\." ) {
   Write-Host "pnpm $pnpm_major is installed." -ForegroundColor Green
} else {
   Write-Host "pnpm $pnpm_major is not installed. Installing now."
   Write-Host 'Running the pnpm installer...'

   npm install -g "pnpm@latest-$pnpm_major"
   Add-DirectoryToPath "$env:APPDATA\npm"
}

Write-Host
Write-Host 'Checking for LLVM...' -ForegroundColor Yellow
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
      Wait-UserInput
      Exit 1
   }

   $oldUninstaller = "$env:SystemDrive\Program Files\LLVM\Uninstall.exe"
   if (Test-Path $oldUninstaller -PathType Leaf) {
      Write-Host 'Uninstalling incompatible LLVM' -ForegroundColor Yellow
      Write-Host 'This may take a while and will have no visual feedback, please wait...'
      &$oldUninstaller /S
   }

   Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\llvm.exe"

   Write-Host "Installing LLVM $llvm_major" -ForegroundColor Yellow
   Write-Host 'This may take a while and will have no visual feedback, please wait...'
   Start-Process -FilePath "$temp\llvm.exe" -Verb RunAs -ArgumentList '/S' -Wait

   Add-DirectoryToPath "$env:SystemDrive\Program Files\LLVM\bin"

   Remove-Item "$temp\llvm.exe"
}

Write-Host
Write-Host 'Checking for protobuf compiler...' -ForegroundColor Yellow
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
      Wait-UserInput
      Exit 1
   }

   $foldername = "$env:LOCALAPPDATA\$([System.IO.Path]::GetFileNameWithoutExtension($fileName))"
   New-Item -Path $foldername -ItemType Directory -ErrorAction SilentlyContinue

   Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\protobuf.zip"

   Write-Host 'Expanding protobuf zip...' -ForegroundColor Yellow

   Expand-Archive "$temp\protobuf.zip" $foldername -ErrorAction SilentlyContinue
   Remove-Item "$temp\protobuf.zip"

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
# Run first time to ensure packages are up to date
cargo metadata --format-version 1 > $null

Write-Host
Write-Host 'Downloading the latest ffmpeg build...' -ForegroundColor Yellow
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
      Wait-UserInput
      Exit 1
   }

   $foldername = "$env:LOCALAPPDATA\$([System.IO.Path]::GetFileNameWithoutExtension($fileName))"

   Start-BitsTransfer -TransferType Download -Source $downloadUri -Destination "$temp\ffmpeg.zip"

   Write-Host 'Expanding ffmpeg zip...' -ForegroundColor Yellow
   # FFmpeg zip contains a subdirectory with the same name as the zip file
   Expand-Archive "$temp\ffmpeg.zip" $env:LOCALAPPDATA -ErrorAction SilentlyContinue
   Remove-Item "$temp\ffmpeg.zip"

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
# Create target\debug folder, continue if already exists
New-Item -Path $projectRoot\target\debug -ItemType Directory -ErrorAction SilentlyContinue
# Copies all .dll required for rust-ffmpeg to target\debug folder
Get-ChildItem "$env:FFMPEG_DIR\bin" -Recurse -Filter *.dll | Copy-Item -Destination "$projectRoot\target\debug"

Write-Host
Write-Host 'Your machine has been setup for Spacedrive development!' -ForegroundColor Green
Write-Host 'You may need to restart your shell to ensure that all environment variables are set!' -ForegroundColor Yellow
Write-Host 'You will need to re-run this script if you use `pnpm clean` or `cargo clean`!' -ForegroundColor Red
Wait-UserInput
