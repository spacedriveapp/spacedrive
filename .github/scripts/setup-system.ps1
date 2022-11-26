param (
	[Parameter()]
	[Switch] $ci
)

$tempPath = "$([System.IO.Path]::GetTempPath())\$([System.Guid]::NewGuid())"

# Check if shell can execute cmdlet
function Test-CommandExists {
	param (
		[string] $command
	)

	return $(if (Get-Command $command -errorAction SilentlyContinue) { $true } else { $false })
}

function Update-EnvironmentVariable {
	param (
		[string]$variableName
	)

	# Write-Host "Updating environment variable: $variableName"

	$value = [System.Environment]::GetEnvironmentVariable($variableName, [System.EnvironmentVariableTarget]::User)

	if ($variableName -ieq "Path") {
		$systemPath = [System.Environment]::GetEnvironmentVariable($variableName, [System.EnvironmentVariableTarget]::Machine)
		$value = "$systemPath;$value"
	}

	[System.Environment]::SetEnvironmentVariable($variableName, $value, [System.EnvironmentVariableTarget]::Process)
}

# Required by Tauri: VS build tools + Windows SDK
function Install-VSTools {
	$downloadUri = "https://aka.ms/vs/17/release/vs_buildtools.exe"
	$executablePath = "$tempPath\vs_buildtools.exe"

	Start-BitsTransfer -Source $downloadUri -Destination $executablePath
	& $executablePath --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --passive | Out-Null
}

function Install-Rustup {
	$downloadUri = "https://win.rustup.rs/"
	$executablePath = "$tempPath\rustup-init.exe"

	Start-BitsTransfer -Source $downloadUri -Destination $executablePath
	Start-Process -FilePath $executablePath -ArgumentList "-y" -PassThru -Wait -Verb RunAs
	Update-EnvironmentVariable "Path"
}

function Install-Pnpm {
	$scriptUri = "https://get.pnpm.io/install.ps1"

	Invoke-WebRequest $scriptUri -useb | Invoke-Expression

	# Working around issue in pnpm that dosen't set %PNPM_HOME% correctly
	$pnpmHome = [System.Environment]::GetEnvironmentVariable("PNPM_HOME", [System.EnvironmentVariableTarget]::User)
	[System.Environment]::SetEnvironmentVariable("PNPM_HOME", $pnpmHome, [System.EnvironmentVariableTarget]::User)
	Update-EnvironmentVariable "PNPM_HOME"
	Update-EnvironmentVariable "Path"
}

New-Item -ItemType Directory -Path $tempPath | Out-Null

try {
	Update-EnvironmentVariable "Path"

	# greeting
	Write-Host "Spacedrive Development Environment Setup" -ForegroundColor Magenta
	Write-Host @"

To set up your machine for Spacedrive development, this script will check for the following and install if necessary:

- Visual Studio build tools
- Cargo
- pnpm
- FFmpeg

"@

	# vs step
	Write-Host "Checking for VS build tools..." -ForegroundColor Yellow
	$vswherePath = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
	$hasVSwhere = Test-Path -Path $vswherePath
	if ($hasVSwhere -eq $false) {
		Write-Host "VS build tools not found. Installing."
		Install-VSTools
	}
	Write-Host "VS build tools are installed."

	# rustup step
	Write-Host "Checking for Cargo..." -ForegroundColor Yellow
	$hasCargo = Test-CommandExists cargo
	if ($hasCargo -eq $false) {
		Write-Host "Cargo not found. Installing."
		Install-Rustup
	}
	Write-Host "Cargo is installed."

	# pnpm step
	Write-Host "Checking for pnpm..." -ForegroundColor Yellow
	$hasPnpm = Test-CommandExists "pnpm"
	if ($hasPnpm -eq $false) {
		Write-Host "pnpm not found. Installing."
		Install-Pnpm
	}
	Write-Host "pnpm is installed."

	Write-Host "Using pnpm to install the latest version of Node..."
	Start-Process -FilePath "pnpm" -ArgumentList "env", "use", "--global", "latest" -Wait -PassThru -Verb RunAs | Out-Null

	# fin
	Write-Host
	Write-Host "Your machine has been set up for Spacedrive development!"
}
finally {
	Remove-Item -Recurse -Force -Path $tempPath
}

# Toolchain (Spacedrive):
#	Cargo (via rustup)
#	pnpm

# Toolchain (Tauri):
#	VS Build Tools 2019
#	Windows 10 SDK

# Dependencies (Spacedrive):
#	FFmpeg