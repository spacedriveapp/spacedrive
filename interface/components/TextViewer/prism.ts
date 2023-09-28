//@ts-nocheck

// WARNING: Import order matters

window.Prism = window.Prism || {};
Prism.manual = true;

import "prismjs";
import './prism.css';

// Languages
// Do not include default ones: markup, html, xml, svg, mathml, ssml, atom, rss, css, clike, javascript, js
import 'prismjs/components/prism-applescript.js';
import 'prismjs/components/prism-bash.js';
import 'prismjs/components/prism-c.js';
import 'prismjs/components/prism-cpp.js';
import 'prismjs/components/prism-ruby.js';
import 'prismjs/components/prism-crystal.js';
import 'prismjs/components/prism-csharp.js';
import 'prismjs/components/prism-css-extras.js';
import 'prismjs/components/prism-csv.js';
import 'prismjs/components/prism-d.js';
import 'prismjs/components/prism-dart.js';
import 'prismjs/components/prism-docker.js';
import 'prismjs/components/prism-go-module.js';
import 'prismjs/components/prism-go.js';
import 'prismjs/components/prism-haskell.js';
import 'prismjs/components/prism-ini.js';
import 'prismjs/components/prism-java.js';
import 'prismjs/components/prism-js-extras.js';
import 'prismjs/components/prism-json.js';
import 'prismjs/components/prism-jsx.js';
import 'prismjs/components/prism-kotlin.js';
import 'prismjs/components/prism-less.js';
import 'prismjs/components/prism-lua.js';
import 'prismjs/components/prism-makefile.js';
import 'prismjs/components/prism-markdown.js';
import 'prismjs/components/prism-markup-templating.js';
import 'prismjs/components/prism-nim.js';
import 'prismjs/components/prism-objectivec.js';
import 'prismjs/components/prism-ocaml.js';
import 'prismjs/components/prism-perl.js';
import 'prismjs/components/prism-php.js';
import 'prismjs/components/prism-powershell.js';
import 'prismjs/components/prism-python.js';
import 'prismjs/components/prism-qml.js';
import 'prismjs/components/prism-r.js';
import 'prismjs/components/prism-rust.js';
import 'prismjs/components/prism-sass.js';
import 'prismjs/components/prism-scss.js';
import 'prismjs/components/prism-solidity.js';
import 'prismjs/components/prism-sql.js';
import 'prismjs/components/prism-swift.js';
import 'prismjs/components/prism-toml.js';
import 'prismjs/components/prism-tsx.js';
import 'prismjs/components/prism-typescript.js';
import 'prismjs/components/prism-typoscript.js';
import 'prismjs/components/prism-vala.js';
import 'prismjs/components/prism-yaml.js';
import 'prismjs/components/prism-zig.js';


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
