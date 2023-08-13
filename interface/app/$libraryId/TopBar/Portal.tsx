import { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useTopBarContext } from './Layout';

export const TopBarPortal = (props: { left?: ReactNode; right?: ReactNode }) => {
	const ctx = useTopBarContext();

	return (
		<>
			{props.left && ctx.left && createPortal(props.left, ctx.left)}
			{props.right && ctx.right && createPortal(props.right, ctx.right)}
		</>
	);
};
