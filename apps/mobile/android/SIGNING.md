# Android Release Signing Configuration

This document explains how to configure release signing for the Spacedrive Android app.

## Overview

Release builds require a signing key to be installed on user devices. Debug builds use a shared debug keystore, but release builds should use a secure, production keystore.

## Environment Variables

The build system looks for the following environment variables:

| Variable | Description |
|----------|-------------|
| `SPACEDRIVE_KEYSTORE_PATH` | Absolute path to your release keystore file (`.jks` or `.keystore`) |
| `SPACEDRIVE_KEYSTORE_PASSWORD` | Password for the keystore |
| `SPACEDRIVE_KEY_ALIAS` | Alias of the signing key within the keystore |
| `SPACEDRIVE_KEY_PASSWORD` | Password for the signing key (often same as keystore password) |

## Setup Instructions

### 1. Generate a Keystore (if you don't have one)

```bash
keytool -genkey -v -keystore spacedrive-release.keystore \
  -alias spacedrive \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000
```

Follow the prompts to set passwords and enter organization details.

### 2. Store the Keystore Securely

- Keep the keystore file in a secure location (NOT in the repository)
- Back up the keystore - losing it means you cannot update your app
- Consider using a secrets manager for CI/CD

### 3. Set Environment Variables

For local development, add to your shell profile (`.bashrc`, `.zshrc`, etc.):

```bash
export SPACEDRIVE_KEYSTORE_PATH="/path/to/spacedrive-release.keystore"
export SPACEDRIVE_KEYSTORE_PASSWORD="your-keystore-password"
export SPACEDRIVE_KEY_ALIAS="spacedrive"
export SPACEDRIVE_KEY_PASSWORD="your-key-password"
```

### 4. Build Release APK

```bash
cd apps/mobile/android
./gradlew assembleRelease
```

The signed APK will be at: `app/build/outputs/apk/release/app-release.apk`

## CI/CD Configuration

For GitHub Actions, add secrets:
- `ANDROID_KEYSTORE_BASE64` - Base64-encoded keystore file
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

Example workflow step:
```yaml
- name: Decode keystore
  run: echo "${{ secrets.ANDROID_KEYSTORE_BASE64 }}" | base64 -d > release.keystore

- name: Build release APK
  env:
    SPACEDRIVE_KEYSTORE_PATH: ${{ github.workspace }}/release.keystore
    SPACEDRIVE_KEYSTORE_PASSWORD: ${{ secrets.ANDROID_KEYSTORE_PASSWORD }}
    SPACEDRIVE_KEY_ALIAS: ${{ secrets.ANDROID_KEY_ALIAS }}
    SPACEDRIVE_KEY_PASSWORD: ${{ secrets.ANDROID_KEY_PASSWORD }}
  run: ./gradlew assembleRelease
```

## Fallback Behavior

If the environment variables are not set or the keystore file doesn't exist, the build falls back to the debug keystore. This allows developers to build release variants without production keys for testing purposes.

## Security Notes

- Never commit keystores or passwords to version control
- Use different keystores for development and production
- Rotate keys if they may have been compromised
- The Play Store requires consistent signing for app updates
