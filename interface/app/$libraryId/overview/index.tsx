import {
	ArrowsIn,
	ArrowsOut,
	ArrowsOutCardinal,
	DotsThreeVertical,
	GearSix
} from '@phosphor-icons/react';
import clsx from 'clsx';
import {
	createElement,
	lazy,
	Suspense,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState
} from 'react';
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
		<div className="mb-2 flex items-center justify-between" data-swapy-handle>
			<div className="text-ink-dull">
				<ArrowsOutCardinal className="size-4" />
			</div>
			<span className="text-sm font-medium text-ink-dull">{title}</span>
			<div className="flex items-center gap-2">
				{expandable && (
					<Button
						size="icon"
						variant="outline"
						onClick={(e: any) => {
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
	title,
	...props
}: {
	children: React.ReactNode;
	className?: string;
	size?: CardSize;
	onSizeChange?: (size: CardSize) => void;
	id: string;
	expandable?: boolean;
	title: string;
	[key: string]: any;
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
			{...props}
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
	const swapyErrorsRef = useRef(0);
	const swapyLastErrorRef = useRef(0);
	const [cardsLoaded, setCardsLoaded] = useState(false);
	const [cardsReady, setCardsReady] = useState(false);
	const cardLoadCountRef = useRef(0);
	const initAttemptRef = useRef(0);
	const cardRefs = useRef<Map<string, HTMLDivElement>>(new Map());

	const enabledCards = useMemo(() => store.cards.filter((card) => card.enabled), [store.cards]);

	const handleSwapyErrorRef = useRef(() => {});

	const handleSwapyError = useCallback(() => {
		const now = Date.now();
		swapyErrorsRef.current++;

		if (swapyErrorsRef.current < 3 && now - swapyLastErrorRef.current > 5000) {
			swapyLastErrorRef.current = now;
			console.log('Reinitializing swapy after error');
			// Add a small delay before reinitializing
			setTimeout(() => {
				initializeSwapy();
			}, 100);
		} else if (swapyErrorsRef.current >= 3) {
			console.error('Too many swapy errors, disabling drag functionality');
		}
	}, []);

	const initializeSwapy = useCallback(() => {
		const container = containerRef.current;
		if (!container || !enabledCards.length || !cardsReady) {
			console.log('Not ready to initialize swapy:', {
				container: !!container,
				enabledCards: enabledCards.length,
				cardsReady,
				attempt: initAttemptRef.current
			});
			return;
		}

		// Verify all card elements exist and are mounted
		const allCardsPresent = enabledCards.every((card) => {
			const element = cardRefs.current.get(card.id);
			return element && element.isConnected;
		});

		if (!allCardsPresent) {
			console.log('Not all cards are mounted yet');
			if (initAttemptRef.current < 3) {
				initAttemptRef.current++;
				setTimeout(() => initializeSwapy(), 100);
			}
			return;
		}

		console.log('Initializing swapy with:', {
			containerWidth: container.offsetWidth,
			containerHeight: container.offsetHeight,
			numCards: enabledCards.length,
			attempt: initAttemptRef.current
		});

		// Clean up previous instance
		if (swapyRef.current) {
			try {
				swapyRef.current.destroy();
			} catch (e) {
				console.error('Error destroying swapy:', e);
			}
			swapyRef.current = null;
		}

		// Force a reflow and wait a frame
		container.offsetHeight;
		requestAnimationFrame(() => {
			try {
				const swapy = createSwapy(container, {
					animation: 'spring',
					swapMode: 'hover',
					continuousMode: true,
					autoScrollOnDrag: true
				});

				swapy.onSwap(({ data }) => {
					try {
						const newOrder = data.array.map((item) => item.itemId).filter(Boolean);
						overviewStore.cards = overviewStore.cards.sort((a, b) => {
							const aIndex = newOrder.indexOf(a.id);
							const bIndex = newOrder.indexOf(b.id);
							if (aIndex === -1) return 1;
							if (bIndex === -1) return -1;
							return aIndex - bIndex;
						});
						swapyErrorsRef.current = 0;
					} catch (e) {
						console.error('Error in swap handler:', e);
						handleSwapyErrorRef.current();
					}
				});

				swapyRef.current = swapy;
				console.log('Swapy initialized successfully');
			} catch (e) {
				console.error('Error initializing swapy:', e);
				handleSwapyErrorRef.current();
			}
		});
	}, [enabledCards.length, cardsReady]);

	const handleCardSizeChange = useCallback((id: string, size: CardSize) => {
		const cardIndex = overviewStore.cards.findIndex((card) => card.id === id);
		if (cardIndex !== -1 && overviewStore.cards[cardIndex]?.id) {
			overviewStore.cards[cardIndex] = {
				...overviewStore.cards[cardIndex],
				size
			};
		}
	}, []);

	const handleCardToggle = useCallback((id: string) => {
		const cardIndex = overviewStore.cards.findIndex((card) => card.id === id);
		if (cardIndex !== -1 && overviewStore.cards[cardIndex]?.id) {
			overviewStore.cards[cardIndex].enabled = !overviewStore.cards[cardIndex].enabled;
		}
	}, []);

	const handleResetCards = useCallback(() => {
		overviewStore.cards = defaultCards;
	}, []);

	useEffect(() => {
		handleSwapyErrorRef.current = handleSwapyError;
	}, [handleSwapyError]);

	const handleCardLoad = useCallback(() => {
		cardLoadCountRef.current++;
		if (cardLoadCountRef.current === enabledCards.length) {
			initAttemptRef.current = 0;
			setTimeout(() => {
				setCardsReady(true);
			}, 100);
		}
	}, [enabledCards.length]);

	// Reset initialization state when cards change
	useEffect(() => {
		setCardsReady(false);
		cardLoadCountRef.current = 0;
		initAttemptRef.current = 0;
		cardRefs.current.clear();
		if (swapyRef.current) {
			try {
				swapyRef.current.destroy();
			} catch (e) {
				console.error('Error destroying swapy:', e);
			}
			swapyRef.current = null;
		}
	}, [enabledCards.length]);

	const renderCard = useCallback(
		(card: CardConfig) => {
			return (
				<div
					key={card.id}
					ref={(el) => {
						if (el) cardRefs.current.set(card.id, el);
						else cardRefs.current.delete(card.id);
					}}
					data-swapy-slot={card.id}
					className={clsx('flex-shrink-0', {
						'w-full sm:w-[calc(50%-8px)] lg:w-[calc(25%-12px)]': card.size === 'small',
						'w-full lg:w-[calc(50%-8px)]': card.size === 'medium',
						'w-full': card.size === 'large'
					})}
					style={{
						minWidth:
							card.size === 'small'
								? '200px'
								: card.size === 'medium'
									? '300px'
									: '400px',
						minHeight:
							card.size === 'small'
								? '150px'
								: card.size === 'medium'
									? '200px'
									: '250px'
					}}
				>
					<div
						data-swapy-item={card.id}
						className="h-full w-full"
						onTransitionEnd={() => {
							console.log(`Card ${card.id} ready`);
							handleCardLoad();
						}}
					>
						<div
							data-swapy-handle
							className="mb-2 flex cursor-grab items-center gap-2 active:cursor-grabbing"
							style={{ minHeight: '30px', width: '100%' }}
						>
							<div className="text-ink-dull">
								<ArrowsOutCardinal className="size-4" />
							</div>
							<span className="text-sm font-medium text-ink-dull">{card.title}</span>
						</div>
						<Card className="flex h-full flex-col overflow-hidden bg-app-box/70 p-4">
							<div className="flex-1 overflow-auto">
								<Suspense fallback={<div>Loading...</div>}>
									<CardWrapper id={card.id} />
								</Suspense>
							</div>
						</Card>
					</div>
				</div>
			);
		},
		[handleCardLoad]
	);

	useEffect(() => {
		if (cardsReady) {
			console.log('Cards ready, initializing swapy');
			initializeSwapy();
		}
	}, [cardsReady, initializeSwapy]);

	useEffect(() => {
		if (swapyRef.current) {
			try {
				swapyRef.current.destroy();
			} catch (e) {
				console.error('Error cleaning up swapy:', e);
			}
			swapyRef.current = null;
		}
	}, []);

	return (
		<div className="relative flex h-full flex-col overflow-y-auto">
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

			<div ref={containerRef} data-overview-container className="grid grid-cols-1 gap-4 p-5">
				{enabledCards.map(renderCard)}
			</div>
		</div>
	);
};
