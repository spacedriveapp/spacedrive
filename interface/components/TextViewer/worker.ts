import Prism from 'prismjs';
import './prism';

// if you are intending to use Prism functions manually, you will need to set:
Prism.manual = true;

// Mapping between extensions and prismjs language identifier
// Only for those that are not already internally resolved by prismjs
// https://prismjs.com/#supported-languages
const languageMapping = Object.entries({
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

export const highlight = (code: string, ext: string) => {
	const language = languageMapping.get(ext) ?? ext;
	const grammar = Prism.languages[language];

	return grammar
		? {
				code: Prism.highlight(code, grammar, language),
				language
		  }
		: null;
};
