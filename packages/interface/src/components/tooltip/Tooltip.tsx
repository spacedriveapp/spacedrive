import * as TooltipPrimitive from '@radix-ui/react-tooltip';

export const Tooltip = ({
	children,
	label,
	position = 'bottom'
}: {
	children: React.ReactNode;
	label: string;
	position?: 'top' | 'right' | 'bottom' | 'left';
}) => {
	return (
		<TooltipPrimitive.Provider>
			<TooltipPrimitive.Root>
				<TooltipPrimitive.Trigger asChild>
					<span>{children}</span>
				</TooltipPrimitive.Trigger>
				<TooltipPrimitive.Content
					side={position}
					className="text-xs  rounded px-2 py-1 mb-[2px] bg-gray-300 dark:!bg-gray-900 dark:text-gray-100"
				>
					<TooltipPrimitive.Arrow className="fill-gray-300 dark:!fill-gray-900" />
					{label}
				</TooltipPrimitive.Content>
			</TooltipPrimitive.Root>
		</TooltipPrimitive.Provider>
	);
};
