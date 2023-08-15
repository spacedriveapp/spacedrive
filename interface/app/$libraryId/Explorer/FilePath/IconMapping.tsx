import * as Icons from '@sd/assets/svgs/ext';
import { type FC as FunctionComponent, type SVGProps } from 'react';
import { type ObjectKindKey } from '@sd/client';

export const IconMapping: Partial<
	Record<ObjectKindKey, Record<string, FunctionComponent<SVGProps<SVGSVGElement>>>>
> = {
	Code: {
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
	}
};
