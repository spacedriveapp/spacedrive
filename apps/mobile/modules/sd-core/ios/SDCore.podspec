#
# You will probs wanna add `use_frameworks! :linkage => :static` into your `ios/Podfile` as well.
#

require 'json'

Pod::Spec.new do |s|
  s.name           = 'SDCore'
  s.version        = '0.0.0'
  s.summary        = 'Spacedrive core for React Native'
  s.description    = 'Spacedrive core for React Native'
  s.author         = 'Oscar Beaumont'
	s.license				 = 'APGL-3.0'
  s.platform       = :ios, '14.0'
  s.source         = { git: 'https://github.com/spacedriveapp/spacedrive' }
	s.homepage		 	 = 'https://www.spacedrive.com'
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_COMPILATION_MODE' => 'wholemodule'
  }

	s.script_phase = {
		:name => 'Build Spacedrive Core!',
		:script => 'env -i SPACEDRIVE_CI=$SPACEDRIVE_CI CONFIGURATION=$CONFIGURATION PLATFORM_NAME=$PLATFORM_NAME ${PODS_TARGET_SRCROOT}/build-rust.sh',
		:execution_position => :before_compile
	}

	s.xcconfig = {
		'LIBRARY_SEARCH_PATHS' => '"' + JSON.parse(`cargo metadata`)["target_directory"].to_s + '"',
		'OTHER_LDFLAGS[sdk=iphoneos*]' => '$(inherited) -lsd_mobile_ios',
		'OTHER_LDFLAGS[sdk=iphonesimulator*]' => '$(inherited) -lsd_mobile_iossim'
	}

  s.source_files = "**/*.{h,m,mm,swift,hpp,cpp}"
	s.module_map = "#{s.name}.modulemap"
end
