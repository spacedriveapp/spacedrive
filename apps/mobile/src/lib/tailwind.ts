import React, { ComponentType } from 'react';
import { create } from 'twrnc';

const tw = create(require(`../../tailwind.config.js`));

function styled<P>(Component: ComponentType<P>, baseStyles?: string) {
	return React.forwardRef<ComponentType<P>, P>(({ style, ...props }: any, ref) =>
		React.createElement(Component as any, {
			...props,
			style: twStyle(baseStyles, style),
			ref
		})
	);
}

// Same as clsx, this works with the eslint plugin (tailwindcss/classnames-order).
const twStyle = tw.style;

tw.style = () => {
	throw new Error('Use twStyle instead of tw.style');
};

export { tw, twStyle, styled };
