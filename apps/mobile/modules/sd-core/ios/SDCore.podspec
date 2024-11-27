require "json"

Pod::Spec.new do |s|
  s.name = "SDCore"
  s.version = "0.0.0"
  s.summary = "Spacedrive core for React Native"
  s.description = "Spacedrive core for React Native"
  s.author = "Spacedrive Technology Inc"
  s.license = "AGPL-3.0"
  s.platform = :ios, "14.0"
  s.source = { git: "https://github.com/spacedriveapp/spacedrive" }
  s.homepage = "https://www.spacedrive.com"
  s.static_framework = true

  s.dependency "ExpoModulesCore"

  s.pod_target_xcconfig = {
    "DEFINES_MODULE" => "YES",
    "SWIFT_COMPILATION_MODE" => "wholemodule",
  }

  s.script_phase = {
    :name => "Build Spacedrive Core!",
    :script => "exec \"${PODS_TARGET_SRCROOT}/build-rust.sh\"",
    :execution_position => :before_compile,
  }

  # Add libraries
  ffmpeg_libraries = [
    "-lmp3lame", "-lsoxr", "-ltheora", "-lopus", "-lvorbisenc", "-lx265",
    "-lpostproc", "-ltheoraenc", "-ltheoradec", "-lde265", "-lvorbisfile",
    "-logg", "-lSvtAv1Enc", "-lvpx", "-lhdr10plus", "-lx264", "-lvorbis",
    "-lzimg", "-lsoxr-lsr", "-liconv", "-lbz2", "-llzma"
  ].join(' ')

  # Add frameworks
  ffmpeg_frameworks = [
    "-framework AudioToolbox",
    "-framework VideoToolbox",
    "-framework AVFoundation",
    "-framework SystemConfiguration",
  ].join(' ')

  s.xcconfig = {
    "LIBRARY_SEARCH_PATHS" => '"' + JSON.parse(`cargo metadata`)["target_directory"].to_s + '"',
    "OTHER_LDFLAGS[sdk=iphoneos*]" => "$(inherited) -lsd_mobile_ios #{ffmpeg_libraries} #{ffmpeg_frameworks}",
    "OTHER_LDFLAGS[sdk=iphonesimulator*]" => "$(inherited) -lsd_mobile_iossim #{ffmpeg_libraries} #{ffmpeg_frameworks}",
  }

  s.source_files = "**/*.{h,m,mm,swift,hpp,cpp}"
  s.module_map = "#{s.name}.modulemap"
end
