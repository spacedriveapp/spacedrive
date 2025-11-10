import clsx from "clsx";
import { Children, cloneElement, isValidElement } from "react";

interface TopBarButtonGroupProps {
	children: React.ReactNode;
	className?: string;
}

export function TopBarButtonGroup({ children, className }: TopBarButtonGroupProps) {
	const childArray = Children.toArray(children);

	return (
		<div
			className={clsx(
				"flex items-center h-8 rounded-full",
				"backdrop-blur-xl border border-sidebar-line/30",
				"bg-sidebar-box/20 overflow-hidden",
				className
			)}
		>
			{childArray.map((child, index) => {
				if (!isValidElement(child)) return child;

				return (
					<div key={index} className="relative flex items-center">
						{/* Clone child and remove rounded corners, border, backdrop */}
						{cloneElement(child as React.ReactElement<any>, {
							className: clsx(
								(child as any).props.className,
								"!rounded-none !border-0 !backdrop-blur-none !bg-transparent",
								"hover:!bg-sidebar-box/30"
							),
						})}
						{/* Divider between buttons (except last) */}
						{index < childArray.length - 1 && (
							<div className="h-5 w-px bg-sidebar-line/30" />
						)}
					</div>
				);
			})}
		</div>
	);
}
