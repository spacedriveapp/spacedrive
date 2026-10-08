// The smallest viewer bundle: a plain DOM module the client mounts for the
// `fake` kind. It gets the element to draw into and a context carrying the
// file, the URL of its bytes and the sidecar URL builder, and returns the
// function that takes it down again.
export function mount(el, ctx) {
	const root = document.createElement("div");
	root.setAttribute("data-fake-viewer", "");
	root.style.cssText =
		"display:flex;flex-direction:column;align-items:center;justify-content:center;height:100%;gap:8px;font-family:system-ui,sans-serif;";

	const name = document.createElement("div");
	name.setAttribute("data-file-name", "");
	name.style.cssText = "font-size:20px;font-weight:600;";
	name.textContent = ctx.file.name + (ctx.file.extension ? "." + ctx.file.extension : "");

	const kind = document.createElement("div");
	kind.style.cssText = "opacity:0.7;font-size:13px;";
	kind.textContent = "Mounted by test-extension's fake_viewer for " +
		(ctx.file.content_kind_name ?? ctx.file.content_kind ?? "unknown");

	root.append(name, kind);
	el.appendChild(root);
	return () => {
		root.remove();
	};
}
