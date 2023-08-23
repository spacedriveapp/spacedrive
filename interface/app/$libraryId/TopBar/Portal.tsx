import { type ReactNode, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { useTopBarContext } from './Layout';

interface Props {
	left?: ReactNode;
	right?: ReactNode;
	noSearch?: boolean;
}
export const TopBarPortal = ({ left, right, noSearch }: Props) => {
	const ctx = useTopBarContext();

	useEffect(() => {
		ctx.setNoSearch(noSearch ?? false);
	}, [ctx, noSearch]);

	return (
		<>
			{left && ctx.left && createPortal(left, ctx.left)}
			{right && ctx.right && createPortal(right, ctx.right)}
		</>
	);
};
