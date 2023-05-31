/**
 * Check if webview can display PDFs
 * https://developer.mozilla.org/en-US/docs/Web/API/Navigator/pdfViewerEnabled
 * https://developer.mozilla.org/en-US/docs/Web/API/Navigator/mimeTypes
 * https://developer.mozilla.org/en-US/docs/Web/API/Navigator/plugins
 */
export const pdfViewerEnabled = () => {
	// pdfViewerEnabled is quite new, Safari only started supporting it in march 2023
	// https://caniuse.com/?search=pdfViewerEnabled
	if ('pdfViewerEnabled' in navigator && navigator.pdfViewerEnabled) return true;

	// This is deprecated, but should be supported on all browsers/webviews
	// https://caniuse.com/mdn-api_navigator_mimetypes
	if (navigator.mimeTypes) {
		if ('application/pdf' in navigator.mimeTypes)
			return !!(navigator.mimeTypes['application/pdf'] as null | MimeType)?.enabledPlugin;
		if ('text/pdf' in navigator.mimeTypes)
			return !!(navigator.mimeTypes['text/pdf'] as null | MimeType)?.enabledPlugin;
	}

	// Last ditch effort
	// https://caniuse.com/mdn-api_navigator_plugins
	return 'PDF Viewer' in navigator.plugins;
};
