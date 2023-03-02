import clsx from 'clsx';
import React from 'react';

const twFactory =
	(element: any) =>
	([newClassNames, ..._]: TemplateStringsArray) =>
		React.forwardRef(({ className, ...props }: any, ref) =>
			React.createElement(element, {
				...props,
				className: clsx(newClassNames, className),
				ref
			})
		);

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
