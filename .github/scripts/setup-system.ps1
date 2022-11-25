param (
	[Parameter()]
	[Switch] $ci
)

# Get temp path
# TODO: files are not cleaned up here there is better solution
$temp = [System.IO.Path]::GetTempPath()

# Check if shell can exexute cmdlet
function Test-CommandExists {
	param ($command)

	return if (Get-Command $command -errorAction SilentlyContinue) { $true } else { $false }
}

# Required by Tauri: VS build tools + Windows SDK
function Install-VSTools {
	$downloadUri = "https://aka.ms/vs/17/release/vs_buildtools.exe"

	Start-BitsTransfer -Source $downloadUri -Destination "$temp\vs_buildtools.exe"
	& "$temp\vs_buildtools.exe" --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows10SDK.19041 --passive | Out-Null
}

function Install-Rustup {
	$downloadUri = "https://win.rustup.rs/"

	Start-BitsTransfer -Source $downloadUri -Destination "$temp\rustup-init.exe"
	Start-Process -FilePath "$temp\rustup-init.exe" -ArgumentList "-y" -PassThru -Wait -Verb RunAs
}

function Install-Pnpm {
	$scriptUri = "https://get.pnpm.io/install.ps1"

	Invoke-WebRequest $scriptUri -useb | Invoke-Expression

	# Working around issue in pnpm that dosen't set %PNPM_HOME% correctly
	$pnpmHome = [System.Environment]::GetEnvironmentVariable("PNPM_HOME", [System.EnvironmentVariableTarget]::User)
	[System.Environment]::SetEnvironmentVariable("PNPM_HOME", $pnpmHome, [System.EnvironmentVariableTarget]::User)
}

Install-Pnpm

# Toolchain (Spacedrive):
#	Cargo (via rustup)
#	pnpm

# Toolchain (Tauri):
#	VS Build Tools 2019
#	Windows 10 SDK

# Dependencies (Spacedrive):
#	FFmpeg