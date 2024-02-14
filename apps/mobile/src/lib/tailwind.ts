import { ComponentType, createElement, forwardRef } from 'react';
import { create } from 'twrnc';
import { Themes } from '@sd/client';

let tw = create(require('../constants/style/tailwind.js')());

export function changeTwTheme(theme: Themes) {
	tw = create(require('../constants/style/tailwind.js')(theme));
}

export function styled<P>(Component: ComponentType<P>, baseStyles?: string) {
	return forwardRef<ComponentType<P>, P>(({ style, ...props }: any, ref) =>
		createElement(Component as any, {
			...props,
			style: twStyle(baseStyles, style),
			ref
		})
	);
}

// Same as clsx, this works with the eslint plugin (tailwindcss/classnames-order).
export const twStyle = tw.style;

tw.style = () => {
	throw new Error('Use twStyle instead of tw.style');
};

export { tw };
