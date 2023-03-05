import WebKit

@_cdecl("reload_webview")
public func reloadWebview(webview: WKWebView) -> () {
	webview.window!.orderOut(webview);
	webview.reload();
	webview.window!.makeKey();
}
