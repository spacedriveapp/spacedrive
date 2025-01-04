# Usage
# .\switch_servers.ps1 dev        # Will prompt for relay server modification
# .\switch_servers.ps1 prod       # Will prompt for relay server modification
# .\switch_servers.ps1 dev -r     # Will automatically modify relay servers
# .\switch_servers.ps1 prod -r    # Will automatically modify relay servers
# .\switch_servers.ps1 dev -s     # Will skip relay server modification without prompting
# .\switch_servers.ps1 prod -s    # Will skip relay server modification without prompting


# File paths
$rustFile = "core/crates/cloud-services/src/lib.rs"
$tsxFile = "interface/util/index.tsx"
$coreFile = "core/src/lib.rs"

# Function to prompt for relay servers change
function Prompt-RelayServers {
    while ($true) {
        $response = Read-Host "Do you want to modify relay servers as well? (y/n)"
        switch ($response.ToLower()) {
            'y' { return $true }
            'n' { return $false }
            default { Write-Host "Please answer y or n." }
        }
    }
}

# Check arguments
if ($args.Count -lt 1 -or $args.Count -gt 2) {
    Write-Host "Usage: <script> <dev|prod> [-r|-s]"
    Write-Host "  -r: Automatically modify relay servers without prompting"
    Write-Host "  -s: Skip relay servers modification without prompting"
    exit 1
}

# Check environment argument
$env = $args[0]
if ($env -notmatch '^(dev|prod)$') {
    Write-Host "Invalid argument. Use 'dev' or 'prod'"
    exit 1
}

# Check flags for relay server handling
$modifyRelay = $false
if ($args.Count -eq 2) {
    switch ($args[1]) {
        '-r' { $modifyRelay = $true }
        '-s' { $modifyRelay = $false }
        default {
            Write-Host "Invalid flag. Use -r or -s"
            exit 1
        }
    }
} else {
    $modifyRelay = Prompt-RelayServers
}

# Function to update file content with regex
function Update-FileContent {
    param (
        [string]$FilePath,
        [string]$Pattern,
        [string]$Replacement
    )

    $content = Get-Content $FilePath -Raw
    $content = [regex]::Replace($content, $Pattern, $Replacement)
    Set-Content -Path $FilePath -Value $content -NoNewline
}

switch ($env) {
    'dev' {
        # Update Rust file
        Update-FileContent $rustFile `
            '^pub const AUTH_SERVER_URL.*' `
            '// pub const AUTH_SERVER_URL: &str = "https://auth.spacedrive.com";'
        Update-FileContent $rustFile `
            '^// pub const AUTH_SERVER_URL.*localhost.*' `
            'pub const AUTH_SERVER_URL: &str = "http://localhost:9420";'

        # Update TypeScript file
        Update-FileContent $tsxFile `
            "^export const AUTH_SERVER_URL.*" `
            "// export const AUTH_SERVER_URL = 'https://auth.spacedrive.com';"
        Update-FileContent $tsxFile `
            "^// export const AUTH_SERVER_URL.*localhost.*" `
            "export const AUTH_SERVER_URL = 'http://localhost:9420';"

        if ($modifyRelay) {
            # Comment out production relay
            Update-FileContent $coreFile `
                '^\s*\.unwrap_or_else\(\|_\| "https://relay\.spacedrive\.com:4433/"\.to_string\(\)\)' `
                '$0 // .unwrap_or_else(|_| "https://relay.spacedrive.com:4433/".to_string())'
            # Uncomment development relay
            Update-FileContent $coreFile `
                '^\s*// \.unwrap_or_else\(\|_\| "http://localhost:8081/"\.to_string\(\)\)' `
                '.unwrap_or_else(|_| "http://localhost:8081/".to_string())'

            # Comment out production pkarr
            Update-FileContent $coreFile `
                '^\s*\.unwrap_or_else\(\|_\| "https://irohdns\.spacedrive\.com/pkarr"\.to_string\(\)\)' `
                '$0 // .unwrap_or_else(|_| "https://irohdns.spacedrive.com/pkarr".to_string())'
            # Uncomment development pkarr
            Update-FileContent $coreFile `
                '^\s*// \.unwrap_or_else\(\|_\| "http://localhost:8080/pkarr"\.to_string\(\)\)' `
                '.unwrap_or_else(|_| "http://localhost:8080/pkarr".to_string())'

            # Comment out production cloud domain
            Update-FileContent $coreFile `
                '^\s*\.unwrap_or_else\(\|_\| "cloud\.spacedrive\.com"\.to_string\(\)\)' `
                '$0 // .unwrap_or_else(|_| "cloud.spacedrive.com".to_string())'
            # Uncomment development cloud domain
            Update-FileContent $coreFile `
                '^\s*// \.unwrap_or_else\(\|_\| "localhost"\.to_string\(\)\)' `
                '.unwrap_or_else(|_| "localhost".to_string())'
        }
    }
    'prod' {
        # Update Rust file
        Update-FileContent $rustFile `
            '^// pub const AUTH_SERVER_URL.*spacedrive.*' `
            'pub const AUTH_SERVER_URL: &str = "https://auth.spacedrive.com";'
        Update-FileContent $rustFile `
            '^pub const AUTH_SERVER_URL.*localhost.*' `
            '// pub const AUTH_SERVER_URL: &str = "http://localhost:9420";'

        # Update TypeScript file
        Update-FileContent $tsxFile `
            "^// export const AUTH_SERVER_URL.*spacedrive.*" `
            "export const AUTH_SERVER_URL = 'https://auth.spacedrive.com';"
        Update-FileContent $tsxFile `
            "^export const AUTH_SERVER_URL.*localhost.*" `
            "// export const AUTH_SERVER_URL = 'http://localhost:9420';"

        if ($modifyRelay) {
            # Uncomment production relay
            Update-FileContent $coreFile `
                '^\s*// \.unwrap_or_else\(\|_\| "https://relay\.spacedrive\.com:4433/"\.to_string\(\)\)' `
                '.unwrap_or_else(|_| "https://relay.spacedrive.com:4433/".to_string())'
            # Comment out development relay
            Update-FileContent $coreFile `
                '^\s*\.unwrap_or_else\(\|_\| "http://localhost:8081/"\.to_string\(\)\)' `
                '// .unwrap_or_else(|_| "http://localhost:8081/".to_string())'

            # Uncomment production pkarr
            Update-FileContent $coreFile `
                '^\s*// \.unwrap_or_else\(\|_\| "https://irohdns\.spacedrive\.com/pkarr"\.to_string\(\)\)' `
                '.unwrap_or_else(|_| "https://irohdns.spacedrive.com/pkarr".to_string())'
            # Comment out development pkarr
            Update-FileContent $coreFile `
                '^\s*\.unwrap_or_else\(\|_\| "http://localhost:8080/pkarr"\.to_string\(\)\)' `
                '// .unwrap_or_else(|_| "http://localhost:8080/pkarr".to_string())'

            # Uncomment production cloud domain
            Update-FileContent $coreFile `
                '^\s*// \.unwrap_or_else\(\|_\| "cloud\.spacedrive\.com"\.to_string\(\)\)' `
                '.unwrap_or_else(|_| "cloud.spacedrive.com".to_string())'
            # Comment out development cloud domain
            Update-FileContent $coreFile `
                '^\s*\.unwrap_or_else\(\|_\| "localhost"\.to_string\(\)\)' `
                '// .unwrap_or_else(|_| "localhost".to_string())'
        }
    }
}
