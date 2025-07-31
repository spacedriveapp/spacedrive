# FFmpeg Bundling Design for Core-New

## Executive Summary

This document outlines the design for bundling FFmpeg with Spacedrive core-new, based on the original implementation's approach. FFmpeg is essential for video thumbnail generation, media metadata extraction, and future transcoding capabilities. The system must handle cross-platform bundling while maintaining a reasonable binary size and ensuring all required codecs are available.

## Background

The original Spacedrive core uses FFmpeg for:
- Video thumbnail generation via `sd-ffmpeg` crate
- Media metadata extraction (duration, codec info, bitrate, etc.)
- Future planned features: video transcoding, format conversion

The implementation uses `ffmpeg-sys-next` (v7.0) which requires FFmpeg libraries to be available at runtime.

## Design Goals

1. **Cross-Platform Support**: Bundle FFmpeg on Windows, macOS, Linux, iOS, and Android
2. **Minimal Size**: Include only necessary codecs and features
3. **Legal Compliance**: Ensure proper licensing (LGPL v3)
4. **Easy Updates**: Simple process to update FFmpeg version
5. **Build Integration**: Seamless integration with Spacedrive build process
6. **Runtime Discovery**: Proper library loading on all platforms
7. **Mobile Optimization**: Efficient battery usage and reduced binary size for mobile

## Implementation Strategy

### Platform-Specific Bundling

#### macOS
- Bundle FFmpeg as a framework in `.deps/Spacedrive.framework`
- Include in Tauri config: `"frameworks": ["../../.deps/Spacedrive.framework"]`
- Use `install_name_tool` to fix library paths for distribution
- Symlink shared libraries during build process (as seen in preprep.mjs)

#### Windows
- Bundle FFmpeg DLLs alongside the executable
- Use static linking where possible to reduce DLL dependencies
- Handle both 64-bit builds (32-bit and ARM not supported per setup.ps1)
- Requires LLVM 15.0.7 for building ffmpeg-sys-next

#### Linux
- Dynamic linking with system FFmpeg where available
- Bundle as fallback in AppImage/Flatpak distributions
- Debian package dependencies: include FFmpeg libraries

#### iOS *(Implemented)*
- **Architecture Support**: Full support for arm64, x86_64 simulator, and arm64 simulator
- **Static Linking**: Uses `CARGO_FEATURE_STATIC=1` for all builds
- **Build Process**: 
  - Separate FFmpeg builds for each architecture stored in `.deps/`:
    - `aarch64-apple-ios` (device)
    - `aarch64-apple-ios-sim` (M1 simulator)
    - `x86_64-apple-ios` (Intel simulator)
  - Libraries are built using `build-rust.sh` which:
    - Sets `FFMPEG_DIR` dynamically based on target architecture
    - Creates universal binaries using `lipo`
    - Symlinks FFmpeg libraries to target directory
- **Pod Configuration**: 
  - Extensive codec support including: mp3lame, opus, vorbis, x264, x265, vpx, av1
  - Links against iOS frameworks: AudioToolbox, VideoToolbox, AVFoundation
  - Libraries linked: libsd_mobile_ios (device) or libsd_mobile_iossim (simulator)
- **Feature Flags**: FFmpeg explicitly enabled in `sd-mobile-core` for iOS targets

#### Android *(Not Currently Implemented)*
- **Current Status**: FFmpeg is NOT enabled for Android builds
- **Build System**: Uses `cargo ndk` with platform API 34
- **Target Architectures**: Primarily arm64-v8a (with optional armeabi-v7a and x86_64)
- **Future Implementation Path**:
  - Add FFmpeg feature flag to Android dependencies in `sd-mobile-core`
  - Bundle pre-built FFmpeg libraries for each Android ABI
  - Update `build.sh` to handle FFmpeg library paths
  - Configure JNI bindings for FFmpeg access from Kotlin/Java

### Build Process Integration

1. **Dependency Download Phase**
   ```bash
   # Add to scripts/preprep.mjs or similar
   async function downloadFFmpeg() {
     const platform = process.platform;
     const arch = process.arch;
     
     // Download pre-built FFmpeg binaries
     const ffmpegVersion = "6.1"; // or latest stable
     const downloadUrl = getFFmpegUrl(platform, arch, ffmpegVersion);
     
     // Extract to .deps directory
     await downloadAndExtract(downloadUrl, ".deps/ffmpeg");
   }
   ```

2. **Cargo Build Configuration**
   ```toml
   # In Cargo.toml or .cargo/config.toml
   [env]
   FFMPEG_DIR = { value = ".deps/ffmpeg", relative = true }
   
   [target.'cfg(target_os = "macos")']
   rustflags = ["-C", "link-arg=-Wl,-rpath,@loader_path/../Frameworks"]
   ```

3. **Feature Flag Management**
   ```toml
   # In core-new/Cargo.toml
   [features]
   default = ["ffmpeg"]
   ffmpeg = ["dep:sd-ffmpeg", "sd-media-processor/ffmpeg"]
   
   # Allow building without FFmpeg for testing
   no-ffmpeg = []
   ```

### FFmpeg Configuration

#### Desktop Minimal Configuration
Minimal FFmpeg build configuration to reduce size:

```bash
./configure \
  --disable-programs \
  --disable-doc \
  --disable-network \
  --enable-shared \
  --disable-static \
  --enable-small \
  --disable-debug \
  --disable-encoders \
  --enable-encoder=libwebp \
  --disable-decoders \
  --enable-decoder=h264,hevc,vp9,av1,mjpeg,png,webp \
  --disable-muxers \
  --disable-demuxers \
  --enable-demuxer=mov,mp4,avi,mkv,webm,image2 \
  --disable-parsers \
  --enable-parser=h264,hevc,vp9,av1 \
  --disable-protocols \
  --enable-protocol=file \
  --disable-filters \
  --enable-filter=scale,thumbnail
```

#### iOS Extended Configuration
iOS build includes extensive codec support for maximum compatibility:

- **Audio Codecs**: MP3 (lame), Opus, Vorbis, AAC
- **Video Codecs**: H.264 (x264), H.265 (x265), VP9, AV1 (SvtAv1Enc), Theora
- **Image Processing**: zimg, HDR10+
- **Audio Processing**: SoXR (high-quality resampling)
- **Hardware Acceleration**: VideoToolbox, AudioToolbox
- **Additional Libraries**: iconv, bzip2, lzma

### Crate Structure

```rust
// crates/ffmpeg/src/lib.rs
#[cfg(feature = "bundled")]
mod bundled {
    use std::env;
    use std::path::PathBuf;
    
    pub fn setup_ffmpeg_paths() {
        #[cfg(target_os = "macos")]
        {
            let framework_path = env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("Frameworks/Spacedrive.framework/Libraries");
            
            env::set_var("DYLD_LIBRARY_PATH", framework_path);
        }
        
        #[cfg(target_os = "windows")]
        {
            // FFmpeg DLLs should be in same directory as exe
            let exe_dir = env::current_exe()
                .unwrap()
                .parent()
                .unwrap();
            
            env::set_var("PATH", format!("{};{}", exe_dir.display(), env::var("PATH").unwrap_or_default()));
        }
        
        #[cfg(target_os = "ios")]
        {
            // iOS uses static linking, no runtime path setup needed
            // Libraries are linked at compile time via build.rs
        }
        
        #[cfg(target_os = "android")]
        {
            // Android will load libraries via System.loadLibrary() in JNI
            // Path setup handled by Android's native library loader
        }
    }
}

pub fn initialize() -> Result<(), Error> {
    #[cfg(feature = "bundled")]
    bundled::setup_ffmpeg_paths();
    
    // Initialize FFmpeg
    unsafe {
        ffmpeg_sys_next::av_log_set_level(ffmpeg_sys_next::AV_LOG_ERROR);
    }
    
    Ok(())
}
```

### Size Optimization Strategies

1. **Codec Selection**: Only include codecs for common formats
2. **Hardware Acceleration**: Optional, platform-specific (VideoToolbox on macOS, NVENC on Windows)
3. **Shared Libraries**: Use shared libraries instead of static linking where possible
4. **Compression**: UPX compress binaries on Windows (with signing considerations)

### Testing Strategy

1. **Binary Validation**
   ```rust
   #[test]
   fn test_ffmpeg_available() {
       assert!(sd_ffmpeg::initialize().is_ok());
       
       // Test basic probe functionality
       let test_file = include_bytes!("../test_data/sample.mp4");
       let metadata = sd_ffmpeg::probe_bytes(test_file).unwrap();
       assert!(metadata.duration > 0);
   }
   ```

2. **Platform CI Tests**
   - GitHub Actions matrix for Windows/macOS/Linux
   - Verify thumbnail generation works
   - Check library loading and paths

### Migration Path from Original Core

1. **Preserve API Compatibility**: Keep same public API in `sd-ffmpeg` crate
2. **Database Schema**: Maintain same FFmpeg metadata tables
3. **Job System Integration**: Create `MediaProcessorJob` similar to original
4. **Progressive Rollout**: Feature flag to toggle between system and bundled FFmpeg

## Implementation Checklist

### Desktop Platforms
- [ ] Create `.deps` directory structure
- [ ] Add FFmpeg download script to build process
- [ ] Update Cargo build configuration
- [ ] Implement platform-specific path setup
- [ ] Create minimal FFmpeg build scripts
- [ ] Add to Tauri bundler configuration
- [ ] Write integration tests
- [ ] Document build process for contributors
- [ ] Add license files to distribution
- [ ] Implement size monitoring in CI

### iOS (Completed in Original Core)
- [x] FFmpeg libraries for all iOS architectures
- [x] Build script (`build-rust.sh`) with architecture detection
- [x] Pod configuration with codec libraries
- [x] Static linking configuration
- [x] Framework linking (AudioToolbox, VideoToolbox, etc.)

### Android (To Be Implemented)
- [ ] Add FFmpeg feature flag to Android build
- [ ] Download/build FFmpeg for Android ABIs
- [ ] Update `build.sh` for FFmpeg paths
- [ ] Configure gradle for native library packaging
- [ ] Implement JNI bindings for FFmpeg access
- [ ] Test on various Android API levels

## Future Considerations

1. **WebAssembly Support**: Investigate FFmpeg.wasm for web version
2. **GPU Acceleration**: Add optional hardware encoding/decoding
3. **Codec Expansion**: Add more formats based on user needs
4. **Plugin System**: Allow users to bring their own FFmpeg build

## References

- Original implementation: `spacedrive/crates/ffmpeg/`
- ffmpeg-sys-next: https://github.com/zmwangx/rust-ffmpeg-sys
- FFmpeg licensing: https://ffmpeg.org/legal.html
- Tauri bundling: https://tauri.app/v1/guides/building/resources