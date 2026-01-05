## [Unreleased]

### Breaking Changes

- **Network identity now derives from device_id instead of device_key**
  - Fixes pairing instability caused by keyring resets
  - All devices must re-pair after this update
  - Network identity is now tied to canonical device identity
  - Added Iroh state persistence for improved reliability

### Fixed

- Devices losing pairing after keyring resets (#XXXX)
- NodeId instability across system updates (#XXXX)
- Missing Iroh state persistence (#XXXX)

