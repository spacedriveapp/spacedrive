import { CaretRight } from "@phosphor-icons/react";
import clsx from "clsx";

interface GroupHeaderProps {
  label: string;
  isCollapsed: boolean;
  onToggle: () => void;
  rightComponent?: React.ReactNode;
  sortableAttributes?: any;
  sortableListeners?: any;
}

export function GroupHeader({
  label,
  isCollapsed,
  onToggle,
  rightComponent,
  sortableAttributes,
  sortableListeners,
}: GroupHeaderProps) {
  return (
    <button
      onClick={onToggle}
      {...(sortableAttributes || {})}
      {...(sortableListeners || {})}
      className="mb-1 flex w-full cursor-default items-center gap-2 px-1 text-tiny font-semibold  tracking-wider opacity-60 text-sidebar-ink-faint hover:text-sidebar-ink"
    >
      <CaretRight
        className={clsx("transition-transform", !isCollapsed && "rotate-90")}
        size={10}
        weight="bold"
      />
      <span>{label}</span>
      {rightComponent}
    </button>
  );
}
