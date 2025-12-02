---
id: VOL-005
title: "Treat Connected iPhone as a Virtual Volume for Direct Import"
status: To Do
assignee: james
parent: VOL-000
priority: High
tags: [feature, import, ios, volume, macos]
whitepaper: "Section 4.3.5"
---

## Description

Implement a feature for the macOS build that detects a physically connected iPhone and presents it as a "virtual volume" within the Spacedrive UI. This will allow users to browse photos and videos directly from the device and import them into a Spacedrive Location without needing to use the system's Photos app.

## Implementation Notes

- [cite_start]The implementation must follow the design specified in **IPHONE_AS_VOLUME_DESIGN.md** [cite: 5561-5575].
- [cite_start]Use Apple's native **ImageCaptureCore** framework via an FFI bridge with the `objc2` crate for all device communication [cite: 5563-5564].
- [cite_start]The connected device should be represented as a temporary, virtual `Volume` in the `VolumeManager`[cite: 5569].
- [cite_start]Browsing should be **on-demand and ephemeral**, converting `ICCameraItem` objects into in-memory `Entry` objects on the fly[cite: 5570].
- [cite_start]The import process will be a new, dedicated **`ImportFromDeviceAction`** that uses the job system to stream file data from the device to the destination location [cite: 5570-5571].
- [cite_start]The application's `Info.plist` must be configured with the "Hardened Runtime" and "USB" entitlements[cite: 5568].

## Acceptance Criteria

- [ ] When an iPhone is connected to a Mac, it appears as a new, browsable volume in Spacedrive.
- [ ] The contents of the iPhone's camera roll (photos and videos) are displayed correctly.
- [ ] A user can select items from the iPhone volume and import them into a standard Spacedrive Location.
- [ ] The import operation shows progress and is resumable, like other Spacedrive jobs.
