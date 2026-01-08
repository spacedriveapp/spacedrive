import clsx from "clsx";
import { forwardRef } from "react";

interface TopBarButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: React.ElementType;
  active?: boolean;
  activeAccent?: boolean; // Use accent color when active
  children?: React.ReactNode;
}

export const TopBarButton = forwardRef<HTMLButtonElement, TopBarButtonProps>(
  (
    { icon: Icon, active, activeAccent, className, children, ...props },
    ref
  ) => {
    return (
      <button
        className={clsx(
          "flex items-center justify-center",
          "h-8 backdrop-blur-xl transition-all",
          "border border-sidebar-line/30",
          children ? "gap-2 rounded-full px-3" : "w-8 rounded-full",
          active && activeAccent
            ? "border-accent/30 bg-accent/20 text-accent"
            : active
              ? "bg-sidebar-box/40 text-sidebar-ink"
              : "bg-sidebar-box/20 text-sidebar-inkDull hover:bg-sidebar-box/30 hover:text-sidebar-ink",
          "active:scale-95",
          className
        )}
        ref={ref}
        {...props}
      >
        {Icon && <Icon className="size-[18px]" weight="bold" />}
        {children && <span className="font-medium text-xs">{children}</span>}
      </button>
    );
  }
);

TopBarButton.displayName = "TopBarButton";
