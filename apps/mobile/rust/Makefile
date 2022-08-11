CARGO_FLAGS=-p sdcore-lib --release --lib
TARGET_DIRECTORY = ../../../target
IOS_UNIVERSAL_LIB = $(TARGET_DIRECTORY)/sdcore-universal-ios.a

# TODO: Remove Makefile and build it in Xcode workflow like Android does with Gradle

.PHONY: setup clean

all: $(IOS_UNIVERSAL_LIB)

setup:
	rustup target add aarch64-apple-ios aarch64-apple-ios-sim
	rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android

clean:
	rm -rf OpenSSL-for-iPhone
	rm -rf $(IOS_UNIVERSAL_LIB)

# Build IOS targets and universal library
$(IOS_UNIVERSAL_LIB): $(wildcard src/*.rs ../../../core/src/*.rs ../../../core/src/*/*.rs)
	cargo build $(CARGO_FLAGS) --features ios --target aarch64-apple-ios
	lipo -create -output $(IOS_UNIVERSAL_LIB) $(TARGET_DIRECTORY)/aarch64-apple-ios/release/libsdcore.a

# Build Android target
# TODO

# todo:
# 	cargo build $(CARGO_FLAGS) --target aarch64-linux-android
# 	cargo build $(CARGO_FLAGS) --target armv7-linux-androideabi
# 	cargo build $(CARGO_FLAGS) --target i686-linux-android