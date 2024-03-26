// We keep these out of the `prism-lazy.ts` as that stuff is lazy-loaded, and this stuff is not.

import { useThemeStore } from '@sd/client';

import oneDarkCss from './one-dark.scss?url';
import oneLightCss from './one-light.scss?url';

// Mapping between extensions and prismjs language identifier
// Only for those that are not already internally resolved by prismjs
// https://prismjs.com/#supported-languages
export const languageMapping = Object.entries({
	applescript: ['scpt', 'scptd'],
	// This is not entirely correct, but better than nothing:
	// https://github.com/PrismJS/prism/issues/3656
	// https://github.com/PrismJS/prism/issues/3660
	sh: ['zsh', 'fish'],
	c: ['h'],
	cpp: ['hpp'],
	js: ['mjs'],
	crystal: ['cr'],
	cs: ['csx'],
	makefile: ['make'],
	nim: ['nims'],
	objc: ['m', 'mm'],
	ocaml: ['ml', 'mli', 'mll', 'mly'],
	perl: ['pl'],
	php: ['php', 'php1', 'php2', 'php3', 'php4', 'php5', 'php6', 'phps', 'phpt', 'phtml'],
	powershell: ['ps1', 'psd1', 'psm1'],
	rust: ['rs']
}).reduce<Map<string, string>>((mapping, [id, exts]) => {
	for (const ext of exts) mapping.set(ext, id);
	return mapping;
}, new Map());

export function WithPrismTheme() {
	const theme = useThemeStore();
	return theme.theme === 'dark' ? (
		<link rel="stylesheet" href={oneDarkCss} />
	) : (
		<link rel="stylesheet" href={oneLightCss} />
	);
}
