import clsx from "clsx";

interface DividerProps {
  className?: string;
}

export function Divider({ className }: DividerProps) {
  return <div className={clsx("mx-2 h-px bg-sidebar-line/50", className)} />;
}
