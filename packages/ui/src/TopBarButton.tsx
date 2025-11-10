import clsx from "clsx";
import { forwardRef } from "react";

interface TopBarButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
	icon?: React.ElementType;
	active?: boolean;
	children?: React.ReactNode;
}

export const TopBarButton = forwardRef<HTMLButtonElement, TopBarButtonProps>(
	({ icon: Icon, active, className, children, ...props }, ref) => {
		return (
			<button
				ref={ref}
				className={clsx(
					"flex items-center justify-center",
					"h-8 backdrop-blur-xl transition-all",
					"border border-sidebar-line/30",
					children ? "px-3 gap-2 rounded-full" : "w-8 rounded-full",
					active
						? "bg-sidebar-box/40 text-sidebar-ink"
						: "bg-sidebar-box/20 text-sidebar-inkDull hover:bg-sidebar-box/30 hover:text-sidebar-ink",
					"active:scale-95",
					className
				)}
				{...props}
			>
				{Icon && <Icon className="size-[18px]" weight="bold" />}
				{children && <span className="text-xs font-medium">{children}</span>}
			</button>
		);
	}
);

TopBarButton.displayName = "TopBarButton";
