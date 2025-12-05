require 'json'

package = JSON.parse(File.read(File.join(__dir__, '../package.json')))

Pod::Spec.new do |s|
  s.name           = 'SDMobileCore'
  s.version        = package['version']
  s.summary        = 'Spacedrive Mobile Core - Embedded Rust core for React Native'
  s.license        = 'GPL-3.0'
  s.authors        = 'Spacedrive Technology Inc.'
  s.homepage       = 'https://spacedrive.com'
  s.platforms      = { :ios => '18.0' }
  s.swift_version  = '5.4'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.source_files = "**/*.{h,m,mm,swift,hpp,cpp}"

  # Build Rust library automatically before compilation
  # Commented out - use `cargo xtask build-mobile` to build manually
  # s.script_phase = {
  #   :name => "Build Spacedrive Mobile Core",
  #   :script => "exec \"${PODS_TARGET_SRCROOT}/build-rust.sh\"",
  #   :execution_position => :before_compile,
  #   :input_files => ["${PODS_TARGET_SRCROOT}/../core/Cargo.toml", "${PODS_TARGET_SRCROOT}/../core/src/lib.rs"],
  #   :output_files => [
  #     "${PODS_TARGET_SRCROOT}/libs/device/libsd_mobile_core.a",
  #     "${PODS_TARGET_SRCROOT}/libs/simulator/libsd_mobile_core.a"
  #   ]
  # }

  # Link the built Rust static library
  libs_dir = File.expand_path('libs', __dir__)

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_COMPILATION_MODE' => 'wholemodule'
  }

  # xcconfig propagates to consuming targets (the main app)
  # This ensures the library is found and linked when building the app
  s.xcconfig = {
    "LIBRARY_SEARCH_PATHS[sdk=iphoneos*]" => "$(inherited) \"#{libs_dir}/device\"",
    "LIBRARY_SEARCH_PATHS[sdk=iphonesimulator*]" => "$(inherited) \"#{libs_dir}/simulator\"",
    "OTHER_LDFLAGS" => "$(inherited) -lsd_mobile_core"
  }
end
