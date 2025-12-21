import { useEffect, useState } from 'react';

import oneDarkCss from './one-dark.scss?url';
import oneLightCss from './one-light.scss?url';

export const languageMapping = Object.entries({
	applescript: ['scpt', 'scptd'],
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
	const [isDark, setIsDark] = useState(() =>
		window.matchMedia('(prefers-color-scheme: dark)').matches
	);

	useEffect(() => {
		const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
		const handleChange = (e: MediaQueryListEvent) => setIsDark(e.matches);

		mediaQuery.addEventListener('change', handleChange);
		return () => mediaQuery.removeEventListener('change', handleChange);
	}, []);

	return isDark ? (
		<link rel="stylesheet" href={oneDarkCss} />
	) : (
		<link rel="stylesheet" href={oneLightCss} />
	);
}
