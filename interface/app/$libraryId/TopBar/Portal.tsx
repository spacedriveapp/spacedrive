import { ReactNode, useEffect } from 'react';
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
			{left && ctx.left.current && createPortal(left, ctx.left.current)}
			{right && ctx.right.current && createPortal(right, ctx.right.current)}
		</>
	);
};
