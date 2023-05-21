import { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useTopBarContext } from './Layout';

interface Props {
	left?: ReactNode;
	right?: ReactNode;
}
export const TopBarPortal = ({ left, right }: Props) => {
	const ctx = useTopBarContext();

	return (
		<>
			{left && ctx.left.current && createPortal(left, ctx.left.current)}
			{right && ctx.right.current && createPortal(right, ctx.right.current)}
		</>
	);
};
