import { DotsThree } from "@phosphor-icons/react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { TopBarButton } from "@sd/ui";
import { useState } from "react";
import type { TopBarItem } from "./Context";

interface OverflowButtonProps {
  items: TopBarItem[];
}

export function OverflowButton({ items }: OverflowButtonProps) {
  const [isOpen, setIsOpen] = useState(false);

  if (items.length === 0) return null;

  return (
    <DropdownMenu.Root onOpenChange={setIsOpen} open={isOpen}>
      <DropdownMenu.Trigger asChild>
        <TopBarButton active={isOpen} icon={DotsThree} />
      </DropdownMenu.Trigger>

      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="start"
          className="z-50 min-w-[180px] rounded-lg border border-app-line bg-app py-1 shadow-2xl"
          sideOffset={8}
        >
          {items.map((item) => {
            const isSimpleAction = !!item.onClick;
            const hasSubmenu = !isSimpleAction;

            if (isSimpleAction) {
              return (
                <DropdownMenu.Item
                  className="cursor-pointer px-3 py-2 text-menu-ink text-sm outline-none transition-colors hover:bg-app-hover/50"
                  key={item.id}
                  onClick={() => item.onClick?.()}
                >
                  {item.label}
                </DropdownMenu.Item>
              );
            }

            return (
              <DropdownMenu.Sub key={item.id}>
                <DropdownMenu.SubTrigger className="flex cursor-pointer items-center justify-between px-3 py-2 text-menu-ink text-sm outline-none transition-colors hover:bg-app-hover/50">
                  <span>{item.label}</span>
                  <span className="text-menu-faint text-xs">▶</span>
                </DropdownMenu.SubTrigger>
                <DropdownMenu.Portal>
                  <DropdownMenu.SubContent className="z-50" sideOffset={8}>
                    {item.submenuContent || item.element}
                  </DropdownMenu.SubContent>
                </DropdownMenu.Portal>
              </DropdownMenu.Sub>
            );
          })}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
