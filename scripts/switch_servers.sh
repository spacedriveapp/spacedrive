#!/bin/bash

## Usage:
# ./switch_servers.sh dev        # Will prompt for relay server modification
# ./switch_servers.sh prod       # Will prompt for relay server modification
# ./switch_servers.sh dev -r     # Will automatically modify relay servers
# ./switch_servers.sh prod -r    # Will automatically modify relay servers
# ./switch_servers.sh dev -s     # Will skip relay server modification without prompting
# ./switch_servers.sh prod -s    # Will skip relay server modification without prompting



# Create cleanup function
cleanup() {
    rm -f "$rust_file-e" "$tsx_file-e" "$core_file-e"
}

# Set trap for cleanup on script exit
trap cleanup EXIT

rust_file="core/crates/cloud-services/src/lib.rs"
tsx_file="interface/util/index.tsx"
core_file="core/src/lib.rs"

# Function to prompt for relay servers change
prompt_relay() {
    while true; do
        read -p "Do you want to modify relay servers as well? (y/n): " yn
        case $yn in
            [Yy]* ) return 0;;
            [Nn]* ) return 1;;
            * ) echo "Please answer y or n.";;
        esac
    done
}

if [ $# -ne 1 ] && [ $# -ne 2 ]; then
    echo "Usage: $0 <dev|prod> [-r|-s]"
    echo "  -r: Automatically modify relay servers without prompting"
    echo "  -s: Skip relay servers modification without prompting"
    exit 1
fi

# Check flags for relay server handling
modify_relay=false
if [ "$2" = "-r" ]; then
    modify_relay=true
elif [ "$2" = "-s" ]; then
    modify_relay=false
elif [ $# -eq 1 ]; then
    prompt_relay && modify_relay=true
fi

case $1 in
    "dev")
        # Update Rust file
        sed -i'' -e 's|^pub const AUTH_SERVER_URL.*|// pub const AUTH_SERVER_URL: \&str = "https:\/\/auth.spacedrive.com";|' "$rust_file"
        sed -i'' -e 's|^// pub const AUTH_SERVER_URL.*localhost.*|pub const AUTH_SERVER_URL: \&str = "http:\/\/localhost:9420";|' "$rust_file"

        # Update TypeScript file
        sed -i'' -e "s|^export const AUTH_SERVER_URL.*|// export const AUTH_SERVER_URL = 'https:\/\/auth.spacedrive.com';|" "$tsx_file"
        sed -i'' -e "s|^// export const AUTH_SERVER_URL.*localhost.*|export const AUTH_SERVER_URL = 'http:\/\/localhost:9420';|" "$tsx_file"

        # Update relay servers if requested
        if [ "$modify_relay" = true ]; then
            # Comment out production relay
            sed -i'' -e 's@^\([[:space:]]*\)\.unwrap_or_else(|_| "https://relay.spacedrive.com:4433/".to_string())@\1// .unwrap_or_else(|_| "https://relay.spacedrive.com:4433/".to_string())@' "$core_file"
            # Uncomment development relay
            sed -i'' -e 's@^\([[:space:]]*\)// \.unwrap_or_else(|_| "http://localhost:8081/".to_string())@\1.unwrap_or_else(|_| "http://localhost:8081/".to_string())@' "$core_file"

            # Comment out production pkarr
            sed -i'' -e 's@^\([[:space:]]*\)\.unwrap_or_else(|_| "https://irohdns.spacedrive.com/pkarr".to_string())@\1// .unwrap_or_else(|_| "https://irohdns.spacedrive.com/pkarr".to_string())@' "$core_file"
            # Uncomment development pkarr
            sed -i'' -e 's@^\([[:space:]]*\)// \.unwrap_or_else(|_| "http://localhost:8080/pkarr".to_string())@\1.unwrap_or_else(|_| "http://localhost:8080/pkarr".to_string())@' "$core_file"

            # Comment out production cloud domain
            sed -i'' -e 's@^\([[:space:]]*\)\.unwrap_or_else(|_| "cloud.spacedrive.com".to_string())@\1// .unwrap_or_else(|_| "cloud.spacedrive.com".to_string())@' "$core_file"
            # Uncomment development cloud domain
            sed -i'' -e 's@^\([[:space:]]*\)// \.unwrap_or_else(|_| "localhost".to_string())@\1.unwrap_or_else(|_| "localhost".to_string())@' "$core_file"
        fi
        ;;
    "prod")
        # Update Rust file
        sed -i'' -e 's|^// pub const AUTH_SERVER_URL.*spacedrive.*|pub const AUTH_SERVER_URL: \&str = "https:\/\/auth.spacedrive.com";|' "$rust_file"
        sed -i'' -e 's|^pub const AUTH_SERVER_URL.*localhost.*|// pub const AUTH_SERVER_URL: \&str = "http:\/\/localhost:9420";|' "$rust_file"

        # Update TypeScript file
        sed -i'' -e "s|^// export const AUTH_SERVER_URL.*spacedrive.*|export const AUTH_SERVER_URL = 'https:\/\/auth.spacedrive.com';|" "$tsx_file"
        sed -i'' -e "s|^export const AUTH_SERVER_URL.*localhost.*|// export const AUTH_SERVER_URL = 'http:\/\/localhost:9420';|" "$tsx_file"

        # Update relay servers if requested
        if [ "$modify_relay" = true ]; then
            # Uncomment production relay
            sed -i'' -e 's@^\([[:space:]]*\)// \.unwrap_or_else(|_| "https://relay.spacedrive.com:4433/".to_string())@\1.unwrap_or_else(|_| "https://relay.spacedrive.com:4433/".to_string())@' "$core_file"
            # Comment out development relay
            sed -i'' -e 's@^\([[:space:]]*\)\.unwrap_or_else(|_| "http://localhost:8081/".to_string())@\1// .unwrap_or_else(|_| "http://localhost:8081/".to_string())@' "$core_file"

            # Uncomment production pkarr
            sed -i'' -e 's@^\([[:space:]]*\)// \.unwrap_or_else(|_| "https://irohdns.spacedrive.com/pkarr".to_string())@\1.unwrap_or_else(|_| "https://irohdns.spacedrive.com/pkarr".to_string())@' "$core_file"
            # Comment out development pkarr
            sed -i'' -e 's@^\([[:space:]]*\)\.unwrap_or_else(|_| "http://localhost:8080/pkarr".to_string())@\1// .unwrap_or_else(|_| "http://localhost:8080/pkarr".to_string())@' "$core_file"

            # Uncomment production cloud domain
            sed -i'' -e 's@^\([[:space:]]*\)// \.unwrap_or_else(|_| "cloud.spacedrive.com".to_string())@\1.unwrap_or_else(|_| "cloud.spacedrive.com".to_string())@' "$core_file"
            # Comment out development cloud domain
            sed -i'' -e 's@^\([[:space:]]*\)\.unwrap_or_else(|_| "localhost".to_string())@\1// .unwrap_or_else(|_| "localhost".to_string())@' "$core_file"
        fi
        ;;
    *)
        echo "Invalid argument. Use 'dev' or 'prod'"
        exit 1
        ;;
esac
