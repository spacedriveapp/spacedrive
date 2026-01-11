import { useState } from "react";
import { DotsThree } from "@phosphor-icons/react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { TopBarButton } from "@sd/ui";
import { TopBarItem } from "./Context";

interface OverflowButtonProps {
	items: TopBarItem[];
}

export function OverflowButton({ items }: OverflowButtonProps) {
	const [isOpen, setIsOpen] = useState(false);

	if (items.length === 0) return null;

	return (
		<DropdownMenu.Root open={isOpen} onOpenChange={setIsOpen}>
			<DropdownMenu.Trigger asChild>
				<TopBarButton
					icon={DotsThree}
					active={isOpen}
				/>
			</DropdownMenu.Trigger>

			<DropdownMenu.Portal>
				<DropdownMenu.Content
					className="min-w-[180px] rounded-lg bg-app border border-app-line shadow-2xl py-1 z-50"
					sideOffset={8}
					align="start"
				>
					{items.map((item) => {
						const isSimpleAction = !!item.onClick;
						const hasSubmenu = !isSimpleAction;

						if (isSimpleAction) {
							return (
								<DropdownMenu.Item
									key={item.id}
									onClick={() => item.onClick?.()}
									className="px-3 py-2 text-sm text-menu-ink hover:bg-app-hover/50 transition-colors outline-none cursor-pointer"
								>
									{item.label}
								</DropdownMenu.Item>
							);
						}

						return (
							<DropdownMenu.Sub key={item.id}>
								<DropdownMenu.SubTrigger className="px-3 py-2 text-sm text-menu-ink hover:bg-app-hover/50 transition-colors flex items-center justify-between outline-none cursor-pointer">
									<span>{item.label}</span>
									<span className="text-menu-faint text-xs">▶</span>
								</DropdownMenu.SubTrigger>
								<DropdownMenu.Portal>
									<DropdownMenu.SubContent
										className="z-50"
										sideOffset={8}
									>
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