use super::native::NSObject;

extern "C" {
	pub fn reload_webview(webview: &NSObject);
}
