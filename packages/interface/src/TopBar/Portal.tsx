import { createPortal } from "react-dom";
import { useTopBar } from "./Context";

interface TopBarPortalProps {
	left?: React.ReactNode;
	right?: React.ReactNode;
}

export function TopBarPortal({ left, right }: TopBarPortalProps) {
	const { leftRef, rightRef } = useTopBar();

	return (
		<>
			{left && leftRef?.current && createPortal(left, leftRef.current)}
			{right && rightRef?.current && createPortal(right, rightRef.current)}
		</>
	);
}