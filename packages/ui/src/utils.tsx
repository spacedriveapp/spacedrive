import clsx from 'clsx';
import React from 'react';

function twFactory(element: any) {
	return ([className, ..._]: TemplateStringsArray) => {
		return restyle(element)(() => className);
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

// eslint-ignore-next-line
export const tw = new Proxy((() => {}) as unknown as TailwindFactory, {
	get: (_, property: string) => twFactory(property),
	apply: (_, __, [el]: [React.ReactElement]) => twFactory(el)
});

export const restyle = <
	T extends
		| string
		| React.FunctionComponent<{ className: string }>
		| React.ComponentClass<{ className: string }>
>(
	element: T
) => {
	return (cls: () => string) =>
		React.forwardRef(({ className, ...props }: any, ref) =>
			React.createElement(element, {
				...props,
				className: clsx(cls(), className),
				ref
			})
		);
};
