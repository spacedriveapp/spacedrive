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
	memo,
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
import { Folder } from '~/components';
import { useLocale } from '~/hooks';

import { SearchContextProvider, SearchOptions, useSearchFromSearchParams } from '../search';
import SearchBar from '../search/SearchBar';
import { TopBarPortal } from '../TopBar/Portal';
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

// Draggable wrapper component
const DraggableCard = memo(
	({
		card,
		onLoad,
		cardRefs
	}: {
		card: CardConfig;
		onLoad: () => void;
		cardRefs: React.MutableRefObject<Map<string, HTMLElement>>;
	}) => {
		return (
			<div
				key={card.id}
				ref={(el) => {
					if (el) cardRefs.current.set(card.id, el);
					else cardRefs.current.delete(card.id);
				}}
				data-swapy-slot={card.id}
				className={clsx('flex-shrink-0', {
					'w-full sm:w-[calc(50%-8px)] lg:w-[calc(33.333%-12px)]': card.size === 'small',
					'w-full lg:w-[calc(66.666%-8px)]': card.size === 'medium',
					'w-full': card.size === 'large'
				})}
				style={{
					height: '250px',
					transition: 'width 300ms ease-in-out'
				}}
			>
				<div
					data-swapy-item={card.id}
					className="h-full w-full transform-gpu will-change-transform"
					style={{
						transition: 'transform 300ms ease-in-out'
					}}
					onTransitionEnd={() => {
						console.log(`Card ${card.id} ready`);
						onLoad();
					}}
				>
					<Card className="flex h-full w-full flex-col overflow-hidden bg-app-box/70">
						<div
							data-swapy-handle
							className="flex cursor-grab items-center gap-2 border-b border-app-line/50 p-3 active:cursor-grabbing"
						>
							<div className="text-ink-dull">
								<ArrowsOutCardinal className="size-4" />
							</div>
							<span className="text-sm font-medium text-ink-dull">{card.title}</span>
						</div>
						<div className="relative flex-1 p-4">
							<div className="absolute inset-0 overflow-auto">
								<div className="min-h-full min-w-full">
									<Suspense fallback={<div>Loading...</div>}>
										<CardWrapper id={card.id} />
									</Suspense>
								</div>
							</div>
						</div>
					</Card>
				</div>
			</div>
		);
	}
);

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
	const cardRefs = useRef<Map<string, HTMLElement>>(new Map());
	const domCheckIntervalRef = useRef<number>();

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

	const [isSwapping, setIsSwapping] = useState(false);
	const swapTimeoutRef = useRef<any>(null);

	const initializationTimeoutRef = useRef<any>(null);
	const lastSuccessfulDimensionsRef = useRef<{ width: number; height: number } | null>(null);
	const retryAttemptsRef = useRef(0);
	const MAX_RETRY_ATTEMPTS = 3;

	const resetInitializationState = useCallback(() => {
		retryAttemptsRef.current = 0;
		if (initializationTimeoutRef.current) {
			clearTimeout(initializationTimeoutRef.current);
			initializationTimeoutRef.current = null;
		}
	}, []);

	const retryInitialization = useCallback(() => {
		if (retryAttemptsRef.current < MAX_RETRY_ATTEMPTS) {
			retryAttemptsRef.current++;
			console.log(
				`Retrying initialization ${retryAttemptsRef.current}/${MAX_RETRY_ATTEMPTS}`
			);
			setTimeout(() => {
				resetInitializationState();
				// Use a new function reference to avoid the circular dependency
				const container = containerRef.current;
				if (!container || !enabledCards.length || !cardsReady) return;

				if (swapyRef.current) {
					try {
						swapyRef.current.destroy();
					} catch (e) {
						console.error('Error destroying swapy:', e);
					}
					swapyRef.current = null;
				}

				initializeSwapy();
			}, 100 * retryAttemptsRef.current);
			return false;
		}
		console.log('Max retry attempts reached, proceeding anyway');
		return true;
	}, [enabledCards.length, cardsReady, resetInitializationState]);

	const verifyElements = useCallback(() => {
		const container = containerRef.current;
		if (!container) return false;

		// Verify all card elements exist and have valid dimensions
		for (const card of enabledCards) {
			const element = cardRefs.current.get(card.id);
			if (
				!element ||
				!element.isConnected ||
				element.offsetWidth === 0 ||
				element.offsetHeight === 0
			) {
				return false;
			}
		}

		// Verify container has valid dimensions
		if (container.offsetWidth === 0 || container.offsetHeight === 0) {
			return false;
		}

		return true;
	}, [enabledCards]);

	const checkDomStability = useCallback(async () => {
		let checksCount = 0;
		const maxChecks = 10;

		return new Promise<boolean>((resolve) => {
			const check = () => {
				const container = containerRef.current;
				if (!container) {
					if (checksCount >= maxChecks) {
						resolve(false);
						return;
					}
					checksCount++;
					requestAnimationFrame(check);
					return;
				}

				const currentDimensions = {
					width: container.offsetWidth,
					height: container.offsetHeight
				};

				if (!verifyElements()) {
					if (checksCount >= maxChecks) {
						console.warn('DOM elements not stable after max checks');
						resolve(false);
						return;
					}
					checksCount++;
					requestAnimationFrame(check);
					return;
				}

				// If dimensions are 0 but we have previous successful dimensions, wait longer
				if (currentDimensions.width === 0 && currentDimensions.height === 0) {
					if (checksCount >= maxChecks) {
						console.warn('Container dimensions not stable after max checks');
						resolve(false);
						return;
					}
					checksCount++;
					requestAnimationFrame(check);
					return;
				}

				// Store successful dimensions for future reference
				lastSuccessfulDimensionsRef.current = currentDimensions;
				resolve(true);
			};

			requestAnimationFrame(check);
		});
	}, [verifyElements]);

	const initializeSwapy = useCallback(async () => {
		try {
			const isStable = await checkDomStability();

			if (!isStable) {
				console.warn('DOM not stable, retrying initialization');
				retryInitialization();
				return;
			}

			const container = containerRef.current;
			if (!container) {
				console.warn('Container not found during initialization');
				retryInitialization();
				return;
			}

			console.log('Initializing swapy with:', {
				containerWidth: container.offsetWidth,
				containerHeight: container.offsetHeight,
				numCards: enabledCards.length,
				attempt: retryAttemptsRef.current
			});

			const swapy = createSwapy(container, {
				animation: 'dynamic',
				swapMode: 'hover',
				continuousMode: true,
				autoScrollOnDrag: true
				// onError: handleSwapyError
			});

			swapyRef.current = swapy;
			console.log('Swapy initialized successfully');
			resetInitializationState();
		} catch (e) {
			console.error('Error initializing swapy:', e);
			retryInitialization();
		}
	}, [
		enabledCards.length,
		checkDomStability,
		retryInitialization,
		resetInitializationState,
		handleSwapyError
	]);

	// Effect to handle initialization
	useEffect(() => {
		if (containerRef.current && enabledCards.length && cardsReady && !isSwapping) {
			initializeSwapy();
		}
	}, [enabledCards.length, cardsReady, isSwapping, initializeSwapy]);

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
		resetInitializationState();
	}, [enabledCards.length, resetInitializationState]);

	// Reset swap state when cards change
	useEffect(() => {
		setIsSwapping(false);
		if (swapTimeoutRef.current) {
			clearTimeout(swapTimeoutRef.current);
		}
		if (swapyRef.current) {
			try {
				swapyRef.current.destroy();
			} catch (e) {
				console.error('Error destroying swapy:', e);
			}
			swapyRef.current = null;
		}
	}, [enabledCards.length]);

	// Cleanup intervals on unmount
	useEffect(() => {
		return () => {
			if (domCheckIntervalRef.current) {
				clearInterval(domCheckIntervalRef.current);
			}
			if (swapTimeoutRef.current) {
				clearTimeout(swapTimeoutRef.current);
			}
		};
	}, []);

	// Cleanup on unmount
	useEffect(() => {
		return () => {
			resetInitializationState();
			if (domCheckIntervalRef.current) {
				cancelAnimationFrame(domCheckIntervalRef.current);
			}
			if (swapyRef.current) {
				try {
					swapyRef.current.destroy();
				} catch (e) {
					console.error('Error destroying swapy:', e);
				}
				swapyRef.current = null;
			}
		};
	}, [resetInitializationState]);

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
						'w-full sm:w-[calc(50%-8px)] lg:w-[calc(33.333%-12px)]':
							card.size === 'small',
						'w-full lg:w-[calc(66.666%-8px)]': card.size === 'medium',
						'w-full': card.size === 'large'
					})}
					style={{
						height: '250px',
						transition: 'width 300ms ease-in-out'
					}}
				>
					<div
						data-swapy-item={card.id}
						className="h-full w-full transform-gpu will-change-transform"
						style={{
							transition: 'transform 300ms ease-in-out'
						}}
						onTransitionEnd={() => {
							console.log(`Card ${card.id} ready`);
							handleCardLoad();
						}}
					>
						<Card className="flex h-full w-full flex-col overflow-hidden bg-app-box/70">
							<div
								data-swapy-handle
								className="flex cursor-grab items-center gap-2 border-b border-app-line/50 p-3 active:cursor-grabbing"
							>
								<div className="text-ink-dull">
									<ArrowsOutCardinal className="size-4" />
								</div>
								<span className="text-sm font-medium text-ink-dull">
									{card.title}
								</span>
							</div>
							<div className="relative flex-1 p-4">
								<div className="absolute inset-0 overflow-auto">
									<div className="min-h-full min-w-full">
										<Suspense fallback={<div>Loading...</div>}>
											<CardWrapper id={card.id} />
										</Suspense>
									</div>
								</div>
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
	const search = useSearchFromSearchParams({ defaultTarget: 'paths' });
	return (
		<SearchContextProvider search={search}>
			<TopBarPortal
				center={<SearchBar />}
				left={
					<div className="flex items-center gap-2">
						<Folder size={22} className="-mt-px" />
						<span className="truncate text-sm font-medium">Overview</span>
					</div>
				}
			/>
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
							<DropdownMenu.Item
								key={card.id}
								onClick={() => handleCardToggle(card.id)}
							>
								<CheckBox checked={card.enabled} />
								{card.title}
							</DropdownMenu.Item>
						))}
						<DropdownMenu.Separator />
						<DropdownMenu.Item onClick={() => handleResetCards()}>
							Reset
						</DropdownMenu.Item>
					</DropdownMenu.Root>
				</div>

				<div
					ref={containerRef}
					className="flex h-full w-full flex-wrap content-start gap-4 overflow-y-auto p-5"
				>
					{enabledCards.map(renderCard)}
				</div>
			</div>
		</SearchContextProvider>
	);
};
