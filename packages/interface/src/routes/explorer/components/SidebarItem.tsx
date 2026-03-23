import clsx from "clsx";
import { NavLink } from "react-router-dom";

interface SidebarItemBaseProps {
  icon: React.ElementType | string;
  label: string;
  weight?: "regular" | "fill" | "bold";
  color?: string;
  className?: string;
}

interface SidebarItemLinkProps extends SidebarItemBaseProps {
  to: string;
  end?: boolean;
  onClick?: (e: React.MouseEvent<HTMLAnchorElement>) => void;
  active?: never;
}

interface SidebarItemButtonProps extends SidebarItemBaseProps {
  to?: never;
  end?: never;
  onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
  active?: boolean;
}

type SidebarItemProps = SidebarItemLinkProps | SidebarItemButtonProps;

function SidebarItemContent({
  icon,
  label,
  isActive,
  weight = "bold",
  color,
}: {
  icon: React.ElementType | string;
  label: string;
  isActive: boolean;
  weight?: "regular" | "fill" | "bold";
  color?: string;
}) {
  const isImageUrl = typeof icon === "string";
  const Icon = isImageUrl ? null : icon;

  return (
    <>
      {color ? (
        <span
          className="mr-2 size-4 rounded-full"
          style={{ backgroundColor: color }}
        />
      ) : isImageUrl ? (
        <img src={icon} alt="" className="mr-2 size-4" />
      ) : (
        Icon && <Icon className="mr-2 size-4" weight={isActive ? "fill" : weight} />
      )}
      <span className="truncate">{label}</span>
    </>
  );
}

function getItemClassName(isActive: boolean, className?: string) {
  return clsx(
    "w-full flex flex-row items-center gap-0.5 truncate rounded-lg px-2 py-1.5 text-sm font-medium tracking-wide outline-none",
    "ring-inset ring-transparent focus:ring-1 focus:ring-accent",
    isActive
      ? "bg-sidebar-selected/40 text-sidebar-ink"
      : "text-sidebar-inkDull hover:text-sidebar-ink",
    className
  );
}

export function SidebarItem(props: SidebarItemProps) {
  const { icon, label, weight = "bold", color, className } = props;

  if (props.to != null) {
    return (
      <NavLink
        to={props.to}
        end={props.end}
        onClick={props.onClick}
        className={({ isActive }) => getItemClassName(isActive, className)}
      >
        {({ isActive }) => (
          <SidebarItemContent
            icon={icon}
            label={label}
            isActive={isActive}
            weight={weight}
            color={color}
          />
        )}
      </NavLink>
    );
  }

  return (
    <button
      onClick={props.onClick}
      className={getItemClassName(props.active ?? false, className)}
    >
      <SidebarItemContent
        icon={icon}
        label={label}
        isActive={props.active ?? false}
        weight={weight}
        color={color}
      />
    </button>
  );
}
