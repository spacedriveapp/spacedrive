import { DragDropContext, Draggable, Droppable } from '@hello-pangea/dnd';
import { ArrowsOutCardinal, DotsThreeVertical, GearSix } from '@phosphor-icons/react';
import clsx from 'clsx';
import { useSnapshot } from 'valtio';
import { Button, Card, DropdownMenu } from '@sd/ui';
import { useLocale } from '~/hooks';

import { CardConfig, overviewStore, type CardSize } from './store';

export interface FileKind {
	kind: number;
	name: string;
	count: bigint;
	total_bytes: bigint;
}

export function OverviewCard({
	children,
	className,
	size = 'medium',
	onSizeChange,
	id,
	dragHandleProps
}: {
	children: React.ReactNode;
	className?: string;
	size?: CardSize;
	onSizeChange?: (size: CardSize) => void;
	id: string;
	dragHandleProps?: any;
}) {
	const { t } = useLocale();

	return (
		<Card
			className={clsx(
				'hover:bg-app-dark-box flex h-[300px] flex-col overflow-hidden bg-app-box/70 p-4 transition-colors',
				{
					'col-span-1 w-full': size === 'small',
					'col-span-2 w-full': size === 'medium',
					'col-span-4 w-full': size === 'large'
				},
				className
			)}
		>
			<div
				className="mb-2 flex cursor-grab items-center justify-between active:cursor-grabbing"
				{...dragHandleProps}
			>
				<div className="text-ink-dull">
					<ArrowsOutCardinal className="size-4" />
				</div>
				<DropdownMenu.Root
					trigger={
						<Button size="icon" variant="outline">
							<DotsThreeVertical className="size-4" />
						</Button>
					}
					side="left"
					sideOffset={5}
					alignOffset={-10}
				>
					<DropdownMenu.Item onClick={() => onSizeChange?.('small')}>
						{t('small')}
					</DropdownMenu.Item>
					<DropdownMenu.Item onClick={() => onSizeChange?.('medium')}>
						{t('medium')}
					</DropdownMenu.Item>
					<DropdownMenu.Item onClick={() => onSizeChange?.('large')}>
						{t('large')}
					</DropdownMenu.Item>
				</DropdownMenu.Root>
			</div>
			{children}
		</Card>
	);
}

export const Component = () => {
	const store = useSnapshot(overviewStore);
	const { t } = useLocale();

	const handleCardSizeChange = (id: string, size: CardSize) => {
		const cardIndex = overviewStore.cards.findIndex((card) => card.id === id);
		if (cardIndex !== -1 && overviewStore.cards[cardIndex]?.id) {
			overviewStore.cards[cardIndex] = {
				...overviewStore.cards[cardIndex],
				size
			};
		}
	};

	const handleCardToggle = (id: string) => {
		const cardIndex = overviewStore.cards.findIndex((card) => card.id === id);
		if (cardIndex !== -1 && overviewStore.cards[cardIndex]?.id) {
			overviewStore.cards[cardIndex].enabled = !overviewStore.cards[cardIndex].enabled;
		}
	};

	const handleDragEnd = (result: any) => {
		if (!result.destination) return;

		const items = Array.from(overviewStore.cards);
		const [reorderedItem] = items.splice(result.source.index, 1);
		if (reorderedItem) {
			items.splice(result.destination.index, 0, reorderedItem);
		}

		overviewStore.cards = items;
	};

	return (
		<div>
			<div className="flex justify-end p-4">
				<DropdownMenu.Root
					trigger={
						<Button size="icon" variant="outline">
							<GearSix className="size-4" />
						</Button>
					}
					side="bottom"
					sideOffset={5}
				>
					{store.cards.map((card) => (
						<DropdownMenu.Item key={card.id} onClick={() => handleCardToggle(card.id)}>
							{card.title}
						</DropdownMenu.Item>
					))}
				</DropdownMenu.Root>
			</div>

			<DragDropContext onDragEnd={handleDragEnd}>
				<Droppable droppableId="overview-cards">
					{(provided) => (
						<div
							{...provided.droppableProps}
							ref={provided.innerRef}
							className="grid grid-cols-4 gap-4 p-4"
						>
							{store.cards
								.filter((card) => card.enabled)
								.map((card, index) => (
									<Draggable key={card.id} draggableId={card.id} index={index}>
										{(provided) => (
											<div
												ref={provided.innerRef}
												{...provided.draggableProps}
												className={clsx('w-full', {
													'col-span-1': card.size === 'small',
													'col-span-2': card.size === 'medium',
													'col-span-4': card.size === 'large'
												})}
											>
												<OverviewCard
													id={card.id}
													size={card.size}
													onSizeChange={(size) =>
														handleCardSizeChange(card.id, size)
													}
													dragHandleProps={provided.dragHandleProps}
												>
													{card.component}
												</OverviewCard>
											</div>
										)}
									</Draggable>
								))}
							{provided.placeholder}
						</div>
					)}
				</Droppable>
			</DragDropContext>
		</div>
	);
};
