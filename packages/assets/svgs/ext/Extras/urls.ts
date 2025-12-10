/*
 * Bearded icon SVG URLs for use in interface package.
 * Auto-generated mapping.
 */

// Use glob to import all SVGs as URLs
const modules = import.meta.glob<string>('./*.svg', {
	eager: true,
	query: '?url',
	import: 'default'
});

// Create a clean mapping: filename -> URL
export const beardedIconUrls: Record<string, string> = {};

Object.keys(modules).forEach(path => {
	// Extract filename without path and extension
	// "./typescript.svg" -> "typescript"
	const name = path.replace('./', '').replace('.svg', '');
	beardedIconUrls[name] = modules[path];
});
