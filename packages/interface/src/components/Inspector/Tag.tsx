import clsx from "clsx";

interface TagProps {
  color: string;
  children: React.ReactNode;
  size?: "sm" | "md";
  className?: string;
}

export function Tag({ color, children, size = "sm", className }: TagProps) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1.5 rounded-full font-medium",
        size === "sm" && "px-2 py-0.5 text-xs",
        size === "md" && "px-2.5 py-1 text-sm",
        className,
      )}
      style={{ backgroundColor: `${color}20`, color }}
    >
      <span
        className={clsx(
          "rounded-full",
          size === "sm" && "size-1.5",
          size === "md" && "size-2",
        )}
        style={{ backgroundColor: color }}
      />
      {children}
    </span>
  );
}
