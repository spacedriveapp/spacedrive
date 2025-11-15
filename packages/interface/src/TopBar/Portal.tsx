import { createPortal } from "react-dom";
import { useTopBar } from "./Context";

interface TopBarPortalProps {
	left?: React.ReactNode;
	center?: React.ReactNode;
	right?: React.ReactNode;
}

export function TopBarPortal({ left, center, right }: TopBarPortalProps) {
	const { leftRef, centerRef, rightRef } = useTopBar();

	return (
		<>
			{left && leftRef?.current && createPortal(left, leftRef.current)}
			{center && centerRef?.current && createPortal(center, centerRef.current)}
			{right && rightRef?.current && createPortal(right, rightRef.current)}
		</>
	);
}
