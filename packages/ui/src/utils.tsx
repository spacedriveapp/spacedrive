import React from 'react';

function twFactory(element: any) {
	return ([className, ..._]: TemplateStringsArray) => {
		const Component = React.forwardRef(({ className: pClassName, ...props }: any, ref) =>
			React.createElement(element, {
				...props,
				className: [className, pClassName],
				ref
			})
		);

		return Component;
	};
}

type ClassnameFactory<T> = (s: TemplateStringsArray) => T;

type TailwindFactory = {
	[K in keyof JSX.IntrinsicElements]: ClassnameFactory<
		React.ForwardRefExoticComponent<JSX.IntrinsicElements[K]>
	>;
} & {
	<T>(c: T): ClassnameFactory<T>;
};

export const tw = new Proxy((() => {}) as unknown as TailwindFactory, {
	get: (_, property: string) => twFactory(property),
	apply: (_, __, [el]: [React.ReactElement]) => twFactory(el)
});
