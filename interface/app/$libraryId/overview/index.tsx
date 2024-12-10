import {
	ArrowsIn,
	ArrowsOut,
	ArrowsOutCardinal,
	DotsThreeVertical,
	GearSix
} from '@phosphor-icons/react';
import clsx from 'clsx';
import { createElement, lazy, Suspense, useEffect, useMemo, useRef, useState } from 'react';
import { createSwapy } from 'swapy';
import { useSnapshot } from 'valtio';
import { Button, Card, CheckBox, DropdownMenu } from '@sd/ui';
import { useLocale } from '~/hooks';

import { CardConfig, defaultCards, overviewStore, type CardSize } from './store';

export interface FileKind {
	kind: number;
	name: string;
	count: bigint;
	total_bytes: bigint;
}

// Define components mapping with component types instead of JSX.Element
const CARD_COMPONENTS: Record<string, React.ComponentType> = {
	'library-stats': lazy(() => import('./cards/LibraryStats')),
	'space-wizard': lazy(() => import('./cards/SpaceWizard')),
	'favorites': lazy(() => import('./cards/FavoriteItems')),
	'device-list': lazy(() => import('./cards/DeviceList')),
	'file-kind-stats': lazy(() => import('./cards/FileKindStats')),
	'recent-files': lazy(() => import('./cards/RecentItems')),
	'recent-locations': lazy(() => import('./cards/RecentLocations')),
	'storage-meters': lazy(() => import('./cards/StorageMeters')),
	'sync-cta': lazy(() => import('./cards/SyncCTA'))
};

interface CardHeadingProps {
	title: string;
	onSizeChange?: (size: CardSize) => void;
	expandable?: boolean;
	isExpanded?: boolean;
	onExpandToggle?: () => void;
}

function CardHeading({
	title,
	onSizeChange,
	expandable,
	isExpanded,
	onExpandToggle
}: CardHeadingProps) {
	const { t } = useLocale();

	const store = useSnapshot(overviewStore);

	const size = useMemo(() => {
		return store.cards.find((card) => card.title === title)?.size;
	}, [store.cards, title]);

	return (
		<div className="mb-2 flex items-center justify-between">
			<div
				className="flex cursor-grab items-center gap-2 active:cursor-grabbing"
				data-swapy-handle
			>
				<div className="text-ink-dull">
					<ArrowsOutCardinal className="size-4" />
				</div>
				<span className="text-sm font-medium text-ink-dull">{title}</span>
			</div>

			<div className="flex items-center gap-2">
				{expandable && (
					<Button
						size="icon"
						variant="outline"
						onClick={(e) => {
							e.stopPropagation();
							onExpandToggle?.();
						}}
					>
						{isExpanded ? (
							<ArrowsIn className="size-4" />
						) : (
							<ArrowsOut className="size-4" />
						)}
					</Button>
				)}
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
						<CheckBox checked={size === 'small'} />
						{t('small')}
					</DropdownMenu.Item>
					<DropdownMenu.Item onClick={() => onSizeChange?.('medium')}>
						<CheckBox checked={size === 'medium'} />
						{t('medium')}
					</DropdownMenu.Item>
					<DropdownMenu.Item onClick={() => onSizeChange?.('large')}>
						<CheckBox checked={false} />
						{t('large')}
					</DropdownMenu.Item>
				</DropdownMenu.Root>
			</div>
		</div>
	);
}

export function OverviewCard({
	children,
	className,
	size = 'medium',
	onSizeChange,
	id,
	expandable,
	title
}: {
	children: React.ReactNode;
	className?: string;
	size?: CardSize;
	onSizeChange?: (size: CardSize) => void;
	id: string;
	expandable?: boolean;
	title: string;
}) {
	const [isExpanded, setIsExpanded] = useState(false);

	return (
		<Card
			className={clsx(
				'flex flex-col overflow-hidden transition-all duration-200 ease-out',
				{
					'fixed bottom-4 left-[calc(180px+1rem)] right-4 top-4 z-50 !h-[calc(100vh-32px)] bg-sidebar/80 backdrop-blur':
						isExpanded,
					'h-[250px] bg-app-box/70 p-4': !isExpanded
				},
				className
			)}
		>
			<CardHeading
				title={title}
				onSizeChange={onSizeChange}
				expandable={expandable}
				isExpanded={isExpanded}
				onExpandToggle={() => setIsExpanded(!isExpanded)}
			/>
			<div className={clsx('flex-1 overflow-auto', isExpanded && 'p-4')}>{children}</div>
		</Card>
	);
}

// Add a wrapper component to handle hot reloading
const CardWrapper = ({ id }: { id: string }) => {
	const CardComponent = CARD_COMPONENTS[id];
	return CardComponent ? <CardComponent /> : null;
};

export const Component = () => {
	const store = useSnapshot(overviewStore);
	const { t } = useLocale();
	const containerRef = useRef<HTMLDivElement>(null);
	const swapyRef = useRef<any>(null);
	const swapyInitialized = useRef(false);

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

	const handleResetCards = () => {
		overviewStore.cards = defaultCards;
	};

	const enabledCards = useMemo(() => store.cards.filter((card) => card.enabled), [store.cards]);

	// Initialize swapy
	useEffect(() => {
		if (!containerRef.current || swapyInitialized.current) return;

		const container = containerRef.current;
		swapyRef.current = createSwapy(container, {
			swapMode: 'hover'
		});

		const handleSwap = ({ data }: { data: any }) => {
			if (!data?.object) return;

			const newOrder = Object.entries(data.object)
				.sort(([a], [b]) => Number(a) - Number(b))
				.map(([_, id]) => id);

			const currentEnabled = [...store.cards].filter((card) => card.enabled);
			const currentDisabled = [...store.cards].filter((card) => !card.enabled);

			const reorderedCards = newOrder
				.map((id) => currentEnabled.find((card) => card.id === id))
				.filter((card): card is CardConfig => card !== undefined);

			if (reorderedCards.length === currentEnabled.length) {
				overviewStore.cards = [...reorderedCards, ...currentDisabled];
			}
		};

		swapyRef.current.onSwap(handleSwap);
		swapyInitialized.current = true;

		return () => {
			if (swapyRef.current) {
				swapyRef.current.destroy();
				swapyRef.current = null;
				swapyInitialized.current = false;
			}
		};
	}, []);

	// Re-initialize swapy when cards are enabled/disabled
	useEffect(() => {
		if (!swapyInitialized.current || !containerRef.current) return;

		swapyRef.current.destroy();
		swapyRef.current = createSwapy(containerRef.current, {
			swapMode: 'hover'
		});
	}, [enabledCards.length]);

	return (
		<div className="relative">
			<div className="absolute right-0 top-0 flex justify-end p-4">
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
							<CheckBox checked={card.enabled} />
							{card.title}
						</DropdownMenu.Item>
					))}
					<DropdownMenu.Separator />
					<DropdownMenu.Item onClick={() => handleResetCards()}>Reset</DropdownMenu.Item>
				</DropdownMenu.Root>
			</div>

			<div className="grid gap-4 p-4" ref={containerRef}>
				{enabledCards.map((card, index) => (
					<div
						key={card.id}
						data-swapy-slot
						className={clsx('w-full', {
							'col-span-1': card.size === 'small',
							'col-span-1 md:col-span-1 xl:col-span-2': card.size === 'medium',
							'col-span-1 sm:col-span-2 lg:col-span-4': card.size === 'large'
						})}
					>
						<div data-swapy-item>
							<div
								data-swapy-handle
								className="flex cursor-grab items-center gap-2 p-2 active:cursor-grabbing"
							>
								<div className="text-ink-dull">
									<ArrowsOutCardinal className="size-4" />
								</div>
								<span className="text-sm font-medium text-ink-dull">
									{card.title}
								</span>
							</div>
							<OverviewCard
								id={card.id}
								size={card.size}
								onSizeChange={(size) => handleCardSizeChange(card.id, size)}
								title={card.title}
								expandable={true}
							>
								<Suspense fallback={<div>Loading...</div>}>
									<CardWrapper id={card.id} />
								</Suspense>
							</OverviewCard>
						</div>
					</div>
				))}
			</div>
		</div>
	);
};
