import WebKit

@_cdecl("reload_webview")
public func reloadWebview(webviewPtr: UnsafePointer<WKWebView>) -> () {
	let webview = webviewPtr.pointee;
	webview.window!.orderOut(webview);
	webview.reload();
	webview.window!.makeKey();
}
