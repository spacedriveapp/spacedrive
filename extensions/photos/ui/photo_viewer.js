// Photos' viewer bundle: the image at its original URL with the file name
// beneath it. The `raw` kind previews through the built-in image renderer
// today; this module is what `preview.viewer: "photo_viewer"` would mount.
export function mount(el, ctx) {
	const root = document.createElement("figure");
	root.style.cssText =
		"display:flex;flex-direction:column;align-items:center;justify-content:center;height:100%;margin:0;gap:12px;";

	const url = ctx.originalUrl ?? ctx.thumbnailUrl;
	if (url) {
		const img = document.createElement("img");
		img.src = url;
		img.alt = ctx.file.name;
		img.draggable = false;
		img.style.cssText = "max-width:100%;max-height:85%;object-fit:contain;";
		root.appendChild(img);
	}

	const caption = document.createElement("figcaption");
	caption.style.cssText = "font-family:system-ui,sans-serif;font-size:14px;opacity:0.8;";
	caption.textContent = ctx.file.name + (ctx.file.extension ? "." + ctx.file.extension : "");
	root.appendChild(caption);

	el.appendChild(root);
	return () => root.remove();
}
