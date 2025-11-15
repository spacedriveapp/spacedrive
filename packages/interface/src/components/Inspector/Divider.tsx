import clsx from "clsx";

interface DividerProps {
  className?: string;
}

export function Divider({ className }: DividerProps) {
  return (
    <div
      className={clsx(
        "h-px bg-sidebar-line/50 mx-2",
        className,
      )}
    />
  );
}
