import { CaretDownIcon, GearSixIcon, PlusIcon } from "@phosphor-icons/react";
import type { Space } from "@sd/ts-client";
import { DropdownMenu } from "@sd/ui";
import clsx from "clsx";
import { useCreateSpaceDialog } from "./CreateSpaceModal";

interface SpaceSwitcherProps {
  spaces: Space[] | undefined;
  currentSpace: Space | undefined;
  onSwitch: (spaceId: string) => void;
}

export function SpaceSwitcher({
  spaces,
  currentSpace,
  onSwitch,
}: SpaceSwitcherProps) {
  const createSpaceDialog = useCreateSpaceDialog;

  return (
    <DropdownMenu.Root
      className="overflow-hidden rounded-lg border border-sidebar-line bg-sidebar-box p-1 shadow-sm"
      trigger={
        <button
          className={clsx(
            "flex w-full items-center gap-1.5 rounded-lg px-2 py-1.5 font-medium text-sm",
            "border border-sidebar-line bg-sidebar-box",
            "text-sidebar-ink hover:bg-sidebar-button",
            "focus:outline-none focus:ring-1 focus:ring-accent",
            "transition-colors",
            !currentSpace && "text-sidebar-inkFaint"
          )}
          type="button"
        >
          <div
            className="size-2 shrink-0 rounded-full"
            style={{ backgroundColor: currentSpace?.color || "#666" }}
          />
          <span className="flex-1 truncate text-left">
            {currentSpace?.name || "Select Space"}
          </span>
          <CaretDownIcon aria-hidden="true" size={12} />
        </button>
      }
    >
      {spaces && spaces.length > 1
        ? spaces.map((space) => (
            <DropdownMenu.Item
              className={clsx(
                "rounded-md px-2 py-1 text-sm",
                space.id === currentSpace?.id
                  ? "bg-accent text-white"
                  : "text-sidebar-ink hover:bg-sidebar-selected"
              )}
              key={space.id}
              onClick={() => onSwitch(space.id)}
            >
              <div className="flex items-center gap-2">
                <div
                  className="size-2 rounded-full"
                  style={{ backgroundColor: space.color }}
                />
                <span>{space.name}</span>
              </div>
            </DropdownMenu.Item>
          ))
        : null}
      {spaces && spaces.length > 1 && (
        <DropdownMenu.Separator className="my-1 border-sidebar-line" />
      )}
      <DropdownMenu.Item
        className="rounded-md px-2 py-1 font-medium text-sidebar-ink text-sm hover:bg-sidebar-selected"
        icon={PlusIcon}
        onClick={() => createSpaceDialog()}
      >
        New Space
      </DropdownMenu.Item>
      <DropdownMenu.Item
        className="rounded-md px-2 py-1 font-medium text-sidebar-ink text-sm hover:bg-sidebar-selected"
        icon={GearSixIcon}
      >
        Space Settings
      </DropdownMenu.Item>
    </DropdownMenu.Root>
  );
}
