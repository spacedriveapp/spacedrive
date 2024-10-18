import { PropsWithChildren, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

import { useTopBarContext } from './Context';

interface Props extends PropsWithChildren {
	left?: ReactNode;
	center?: ReactNode;
	right?: ReactNode;
}
export const TopBarPortal = ({ left, center, right, children }: Props) => {
	const ctx = useTopBarContext();

	return (
		<>
			{left && ctx.left && createPortal(left, ctx.left)}
			{center && ctx.center && createPortal(center, ctx.center)}
			{right && ctx.right && createPortal(right, ctx.right)}
			{children && ctx.children && createPortal(children, ctx.children)}
		</>
	);
};
