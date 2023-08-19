import { ReactNode } from 'react';

type UseCondition<TProps extends object> = () => TProps | null;

export class ConditionalItem<TProps extends object> {
	// Named like a hook to please eslint
	useCondition: UseCondition<TProps>;
	// Capital 'C' to please eslint + make rendering after destructuring easier
	Component: React.FC<TProps>;

	constructor(public args: { useCondition: UseCondition<TProps>; Component: React.FC<TProps> }) {
		this.useCondition = args.useCondition;
		this.Component = args.Component;
	}
}

export interface ConditionalGroupProps {
	items: ConditionalItem<any>[];
	children?: (children: ReactNode) => ReactNode;
}

/**
 * Takes an array of `ConditionalItem` and attempts to render them all,
 * returning `null` if all conditions are `null`.
 *
 * @param items An array of `ConditionalItem` to render.
 * @param children An optional render function that can be used to wrap the rendered items.
 */
export const Conditional = ({ items, children }: ConditionalGroupProps) => {
	const itemConditions = items.map((item) => item.useCondition());

	if (itemConditions.every((c) => c === null)) return null;

	const renderedItems = (
		<>
			{itemConditions.map((props, i) => {
				if (props === null) return null;
				const { Component } = items[i]!;
				return <Component key={i} {...props} />;
			})}
		</>
	);

	return <>{children ? children(renderedItems) : renderedItems}</>;
};
