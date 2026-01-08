import clsx from "clsx";

interface SidebarItemProps {
  icon: React.ElementType | string;
  label: string;
  active?: boolean;
  weight?: "regular" | "fill" | "bold";
  color?: string;
  onClick?: () => void;
  className?: string;
}

export function SidebarItem({
  icon,
  label,
  active,
  weight = "bold",
  color,
  onClick,
  className,
}: SidebarItemProps) {
  const isImageUrl = typeof icon === "string";
  const Icon = isImageUrl ? null : icon;

  return (
    <button
      className={clsx(
        "flex w-full flex-row items-center gap-0.5 truncate rounded-lg px-2 py-1.5 font-medium text-sm tracking-wide outline-none",
        "ring-transparent ring-inset focus:ring-1 focus:ring-accent",
        active
          ? "bg-sidebar-selected/40 text-sidebar-ink"
          : "text-sidebar-inkDull hover:text-sidebar-ink",
        className
      )}
      onClick={onClick}
    >
      {color ? (
        <span
          className="mr-2 size-4 rounded-full"
          style={{ backgroundColor: color }}
        />
      ) : isImageUrl ? (
        <img alt="" className="mr-2 size-4" src={icon} />
      ) : (
        Icon && (
          <Icon className="mr-2 size-4" weight={active ? "fill" : weight} />
        )
      )}
      <span className="truncate">{label}</span>
    </button>
  );
}
