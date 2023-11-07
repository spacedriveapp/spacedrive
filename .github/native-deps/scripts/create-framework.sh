#!/usr/bin/env bash

set -xeuo pipefail

case "$TARGET" in
  *darwin*) ;;
  *)
    echo "Framework creation is only for macOS" >&2
    exit 0
    ;;
esac

_version=0.1

# Create Spacedrive.framework
# https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPFrameworks/Concepts/FrameworkAnatomy.html
_framework="Spacedrive.framework"

# Create basic structure
mkdir -p "${OUT:?Missing out dir}/${_framework}/Versions/A/Resources"

# Move libs to Framework
mv "${OUT}/lib" "${OUT}/${_framework}/Versions/A/Libraries"

# Fix linker load path for each library and it's dependency
while IFS= read -r _lib; do
  # Loop through each of the library's dependencies
  for _dep in $(otool -L "$_lib" | tail -n+3 | awk '{print $1}'); do
    case "$_dep" in
      "${OUT}/lib/"*) # One of our built libraries
        # Change the dependency linker path so it loads it from the same directory as the library
        install_name_tool -change "$_dep" "@loader_path/${_dep#"${OUT}/lib/"}" "$_lib"
        ;;
      *) # Ignore system libraries
        continue
        ;;
    esac
  done

  # Update the library's own id
  if ! install_name_tool -id "@executable_path/../Frameworks/${_framework}/Libraries/$(basename "$_lib")" "$_lib"; then
    # Some libraries have a header pad too small, so use a relative path instead
    install_name_tool -id "./$(basename "$_lib")" "$_lib"
  fi
done < <(find "${OUT}/${_framework}/Versions/A/Libraries" -type f -name '*.dylib')

# Move headers to Framework
mv "${OUT}/include" "${OUT}/${_framework}/Versions/A/Headers"

# Move licenses to Framework
mv "${OUT}/licenses" "${OUT}/${_framework}/Versions/A/Resources/Licenses"

# Create required framework symlinks
ln -s A "${OUT}/${_framework}/Versions/Current"
ln -s Versions/Current/Headers "${OUT}/${_framework}/Headers"
ln -s Versions/Current/Resources "${OUT}/${_framework}/Resources"
ln -s Versions/Current/Libraries "${OUT}/${_framework}/Libraries"
ln -s Versions/Current/Spacedrive "${OUT}/${_framework}/Spacedrive"

# Symlink framework directories back to our original layout
ln -s "${_framework}/Headers" "${OUT}/include"
ln -s "${_framework}/Libraries" "${OUT}/lib"
ln -s "${_framework}/Resources/Licenses" "${OUT}/licenses"

# Framework Info.plist (based on macOS internal OpenGL.framework Info.plist)
cat <<EOF >"${OUT}/${_framework}/Versions/Current/Resources/Info.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>Spacedrive</string>
    <key>CFBundleGetInfoString</key>
    <string>Spacedrive Native Dependencies ${_version}</string>
    <key>CFBundleIdentifier</key>
    <string>com.spacedrive</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Spacedrive</string>
    <key>CFBundlePackageType</key>
    <string>FMWK</string>
    <key>CFBundleShortVersionString</key>
    <string>${_version}</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>CFBundleVersion</key>
    <string>${_version}</string>
    <key>NSHumanReadableCopyright</key>
	  <string>Copyright (c) 2021-present Spacedrive Technology Inc.</string>
</dict>
</plist>
EOF

cat <<EOF >"${OUT}/${_framework}/Versions/Current/Resources/version.plist"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
        <key>BuildVersion</key>
        <string>1</string>
        <key>CFBundleShortVersionString</key>
        <string>${_version}</string>
        <key>CFBundleVersion</key>
        <string>${_version}</string>
        <key>ProjectName</key>
        <string>Spacedrive</string>
        <key>SourceVersion</key>
        <string>$(date '+%Y%m%d%H%M%S')</string>
</dict>
</plist>
EOF
