import { type ReactNode } from 'react';
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
			{left && ctx.left && createPortal(left, ctx.left)}
			{right && ctx.right && createPortal(right, ctx.right)}
		</>
	);
};
