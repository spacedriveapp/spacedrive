fn main() {
	#[cfg(all(not(target_os = "windows"), feature = "ai-models"))]
	// This is required because libonnxruntime.so is incorrectly built with the Initial Executable (IE) thread-Local storage access model by zig
	// https://docs.oracle.com/cd/E23824_01/html/819-0690/chapter8-20.html
	// https://github.com/ziglang/zig/issues/16152
	// https://github.com/ziglang/zig/pull/17702
	// Due to this, the linker will fail to dlopen libonnxruntime.so because it runs out of the static TLS space reserved after initial load
	// To workaround this problem libonnxruntime.so is added as a dependency to the binaries, which makes the linker allocate its TLS space during initial load
	println!("cargo:rustc-link-lib=onnxruntime");
}
