import * as Icons from '@sd/assets/svgs/ext';
import React from 'react';

export const IconMapping: Record<string, React.FC<React.SVGProps<SVGSVGElement>>> = {
	rs: Icons.rust,
	go: Icons.go,
	html: Icons.html,
	css: Icons.css,
	scss: Icons.scss,
	js: Icons.js,
	jsx: Icons.js,
	ts: Icons.tsx,
	tsx: Icons.tsx,
	vue: Icons.vue,
	swift: Icons.swift,
	php: Icons.php,
	py: Icons.python,
	rb: Icons.ruby,
	sh: Icons.shell
};
