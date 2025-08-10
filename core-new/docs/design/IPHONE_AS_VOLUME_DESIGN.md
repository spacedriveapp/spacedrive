# Design Document: iPhone as a Volume for Direct Import

## 1. Overview

This document outlines the design for a new feature enabling Spacedrive to detect a physically connected iPhone on macOS and treat it as a "virtual volume." This will allow users to browse the photos and videos on their device directly within the Spacedrive UI and import them into any Spacedrive Location.

This feature is specifically for accessing the **connected device as a camera** and does not interact with the user's system-wide Apple Photos library or iCloud Photos. The implementation will use Apple's official `ImageCaptureCore` framework, ensuring a secure and stable integration.

## 2. Design Principles

- **Native Integration:** Use official, recommended Apple APIs (`ImageCaptureCore`) for all device communication.
- **User Consent First:** All access to the device will be gated by the standard macOS user permission prompts. The user is always in control.
- **Read-Only Source:** The iPhone's storage will be treated as a read-only source. The import process is non-destructive and never modifies the contents of the source device.
- **VDFS Consistency:** The feature will integrate seamlessly with Spacedrive's existing architectural patterns, including the `Volume`, `Entry`, and `Action` / `Job` systems.

## 3. Architecture

The architecture is centered around a new, platform-specific service that acts as a bridge between Spacedrive's core logic and Apple's native frameworks.

```
┌───────────────────────────┐      ┌───────────────────────────┐
│      Spacedrive Core      │      │   macOS Native Frameworks   │
│                           │      │                           │
│  ┌─────────────────────┐  │      │  ┌──────────────────────┐ │
│  │   Volume Manager    │  │      │  │  ImageCaptureCore    │ │
│  └─────────────────────┘  │      │  └──────────────────────┘ │
│  ┌─────────────────────┐  │      │                           │
│  │     Action/Job      │  │      │                           │
│  │       System        │  │      │                           │
│  └─────────────────────┘  │      │                           │
│            ▲              │      │              ▲            │
│            │              │      │              │            │
│  ┌─────────┴─────────────┐  │      │  ┌───────────┴──────────┐ │
│  │ iPhoneDeviceService   │◄─────┼─────►│ FFI Bridge (objc2)   │ │
│  │    (macOS only)       │  │      │  └──────────────────────┘ │
│  └─────────────────────┘  │      │                           │
└───────────────────────────┘      └───────────────────────────┘
                                               │
                                               ▼
                                     ┌──────────────────┐
                                     │ Connected iPhone │
                                     └──────────────────┘
```

### 3.1. The `iPhoneDeviceService` (macOS only)

This new service will be the core of the implementation.

-   **Technology:** It will be written in Rust and use the `objc2` family of crates to create a Foreign Function Interface (FFI) bridge to the Objective-C `ImageCaptureCore` framework.
-   **Permissions:** The final Spacedrive application bundle will need to include an `Info.plist` file with the "Hardened Runtime" capability enabled, specifically requesting the "USB" entitlement. The service will also be responsible for triggering the user permission dialog to access the device.
-   **Lifecycle:** The service will run a device browser (`ICDeviceBrowser`) in a background task to listen for device connection and disconnection events, allowing Spacedrive to react instantly when an iPhone is plugged in or removed.

### 3.2. The "Virtual Volume" Model

A connected iPhone will be represented as a temporary, virtual `Volume` in Spacedrive.

-   **Appearance:** It will appear in the UI alongside other volumes like hard drives and network shares, but with a distinct icon (e.g., a phone icon).
-   **Identity:** The volume's unique identifier will be the UUID provided by `ImageCaptureCore` for the `ICCameraDevice`. It will not have a traditional filesystem mount path.
-   **Lifecycle:** The `iPhoneDeviceService` will create this virtual volume when a device is connected and remove it (or mark it as offline) when the device is disconnected.

### 3.3. On-Demand, Ephemeral Browsing

To avoid indexing the entire contents of the phone, browsing will be done on-demand.

-   **User Flow:** When the user selects the "iPhone" volume in the UI, a live query is sent to the `iPhoneDeviceService`.
-   **Live Query:** The service opens a session with the `ICCameraDevice` and fetches the list of media items (`ICCameraItem` objects).
-   **Ephemeral Entries:** This list is then translated on-the-fly into temporary, in-memory Spacedrive `Entry` objects. These ephemeral entries will use a special `SdPath` format to uniquely identify them.
    -   **URI Format:** `sd://iphone-camera/{device_uuid}/item/{item_id}`

### 3.4. The `ImportFromDeviceAction`

The import process will be a new, dedicated `Action` that leverages the existing job system.

-   **Trigger:** The user selects one or more ephemeral photo/video entries and a standard destination `Location` (e.g., a folder on their NAS).
-   **Action Definition:** A new, generic `ImportFromDeviceAction` will be created.
    ```rust
    pub struct ImportFromDeviceAction {
        pub source_device_id: Uuid,
        pub source_item_ids: Vec<String>, // The native item IDs from ImageCaptureCore
        pub destination_location_id: Uuid,
        // ... other options like "delete after import" (if API supports it)
    }
    ```
-   **Job Execution (`ImportJob`):**
    1.  The `ActionManager` dispatches an `ImportJob` from the action.
    2.  The job calls the `iPhoneDeviceService`, passing the list of item IDs to download.
    3.  The service uses the `ImageCaptureCore` function `requestDownload(for:options:...)` to request the original, full-resolution file data.
    4.  The service streams the file data directly to a temporary location within the final destination.
    5.  Once the file is successfully written, it is moved to its final place in the destination `Location`.
    6.  Spacedrive's standard `LocationWatcher` and `Indexer` will then see a new file, and it will be indexed, hashed, and added to the VDFS like any other file.

## 4. Implementation Plan

### Phase 1: FFI Foundation & Device Discovery
-   **Goal:** Make a connected iPhone appear and disappear in the Spacedrive UI as a virtual volume.
-   **Tasks:**
    1.  Integrate `objc2` crates.
    2.  Configure the application's `Info.plist` with the required entitlements.
    3.  Implement the `iPhoneDeviceService` with the `ICDeviceBrowser` to detect device connections.
    4.  Implement the logic to create and remove the virtual `Volume` in the `VolumeManager`.

### Phase 2: On-Demand Browsing
-   **Goal:** Allow users to see the contents of their connected iPhone.
-   **Tasks:**
    1.  Implement the logic to open a session with an `ICCameraDevice`.
    2.  Fetch the list of `ICCameraItem`s.
    3.  Implement the translation layer that converts `ICCameraItem`s into ephemeral Spacedrive `Entry` objects for the UI.

### Phase 3: Import Workflow
-   **Goal:** Allow users to copy files from their iPhone into Spacedrive.
-   **Tasks:**
    1.  Define the `ImportFromDeviceAction` and `ImportJob` structs.
    2.  Implement the file download logic in the `iPhoneDeviceService` using `requestDownload`.
    3.  Integrate the download stream with the job system to write the file to its final destination.
    4.  Add progress reporting to the job based on `ImageCaptureCore`'s delegate callbacks.

### Phase 4: UI/UX Polish
-   **Goal:** Create a seamless and intuitive user experience.
-   **Tasks:**
    1.  Design a custom icon for the iPhone virtual volume.
    2.  Build the UI for browsing photos and selecting an import destination.
    3.  Integrate job progress indicators (progress bars, notifications) for the import process.

## 5. Security & Privacy
-   **Permissions:** All access to the iPhone is explicitly gated by the standard macOS user consent dialog. The application cannot access the device until the user approves.
-   **Read-Only:** The entire process is read-only. No data on the iPhone is ever modified or deleted by Spacedrive (unless a "delete after import" feature is explicitly added and used).
-   **Native APIs:** By using `ImageCaptureCore`, we are using Apple's blessed, secure, and stable method for this type of interaction.
