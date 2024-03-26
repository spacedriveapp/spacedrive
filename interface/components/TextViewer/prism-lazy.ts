//@ts-nocheck

// WARNING: Import order matters

window.Prism = window.Prism || {};
window.Prism.manual = true;

// This import must be first, to ensure that the `Prism` global is available before importing its language plugins
import "prismjs";

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

export { highlightElement } from 'prismjs';
