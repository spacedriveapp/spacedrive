import type { Icon } from "@phosphor-icons/react";
import clsx from "clsx";

interface SectionProps {
  title: string;
  icon?: Icon;
  children: React.ReactNode;
  className?: string;
}

export function Section({
  title,
  icon: Icon,
  children,
  className,
}: SectionProps) {
  return (
    <div className={clsx("space-y-3", className)}>
      <div className="flex items-center gap-2 px-2">
        {Icon && <Icon className="size-4 text-accent" weight="bold" />}
        <span className="font-semibold text-sidebar-inkFaint text-xs uppercase tracking-wider">
          {title}
        </span>
      </div>
      <div className="space-y-2 px-2">{children}</div>
    </div>
  );
}
