import {
	ArrowLeft,
	ArrowRight,
	ClockCounterClockwise,
	DotsThree
} from '@phosphor-icons/react';
import { Popover, usePopover, SearchBar, CircleButton, CircleButtonGroup, SelectPill, OptionList, OptionListItem, Button } from '@spaceui/primitives';
import {ReactQueryDevtools} from '@tanstack/react-query-devtools';
import {useEffect, useState} from 'react';
import {Outlet, useLocation, useNavigate} from 'react-router-dom';
import {
	agents,
	isMacOS,
	primaryItems,
	projects,
	useSpacebot
} from './SpacebotContext';

function SidebarHistoryItem({
	conversation,
	isActive,
	onClick
}: {
	conversation: {id: string; title: string; last_message_preview?: string};
	isActive: boolean;
	onClick: () => void;
}) {
	return (
		<button
			onClick={onClick}
			className={`flex w-full items-start rounded-lg px-3 py-2 text-left transition-colors ${
				isActive ? 'bg-sidebar-selected/40' : 'hover:bg-sidebar-box'
			}`}
		>
			<div>
				<div className="text-sidebar-ink text-sm font-medium">
					{conversation.title}
				</div>
				<div className="text-sidebar-inkDull line-clamp-2 text-xs">
					{conversation.last_message_preview ?? 'No messages yet'}
				</div>
			</div>
		</button>
	);
}

function SidebarHistory() {
	const location = useLocation();
	const {
		conversations,
		conversationsLoading,
		conversationsError,
		search,
		navigateToConversation
	} = useSpacebot();

	if (conversationsLoading) {
		return (
			<div className="text-sidebar-inkDull px-3 py-2 text-xs">
				Loading conversations...
			</div>
		);
	}

	if (conversationsError) {
		return (
			<div className="text-sidebar-inkDull px-3 py-2 text-xs">
				Could not load conversations.
			</div>
		);
	}

	const filtered = conversations.filter((conversation) => {
		if (!search.trim()) return true;
		const query = search.toLowerCase();
		return (
			conversation.title.toLowerCase().includes(query) ||
			conversation.last_message_preview?.toLowerCase().includes(query)
		);
	});

	if (filtered.length === 0) {
		return (
			<div className="text-sidebar-inkDull px-3 py-2 text-xs">
				No conversations yet.
			</div>
		);
	}

	// Extract the conversation ID from the pathname exactly
	const pathname = location.pathname;
	const conversationPathMatch = pathname.match(/\/conversation\/(.+)$/);
	const activeConversationId = conversationPathMatch
		? decodeURIComponent(conversationPathMatch[1])
		: null;

	return filtered.map((conversation) => {
		const isActive = activeConversationId === conversation.id;
		return (
			<SidebarHistoryItem
				key={conversation.id}
				conversation={conversation}
				isActive={isActive}
				onClick={() => navigateToConversation(conversation.id)}
			/>
		);
	});
}

export function SpacebotLayout() {
	const location = useLocation();
	const navigate = useNavigate();
	const {
		search,
		setSearch,
		selectedAgent,
		setSelectedAgent,
		currentAgent,
		agentSelector,
		navigateToChat
	} = useSpacebot();

	const isChatActive = location.pathname.startsWith('/spacebot/chat');

	// Navigation history for back/forward buttons
	const [historyStack, setHistoryStack] = useState<string[]>([
		location.pathname
	]);
	const [currentIndex, setCurrentIndex] = useState(0);

	// Update history when location changes (from user navigation)
	useEffect(() => {
		setHistoryStack((prev) => {
			// If we're not at the end of the stack, truncate it
			const trimmed = prev.slice(0, currentIndex + 1);
			// Only add if it's different from the current location
			if (trimmed[trimmed.length - 1] !== location.pathname) {
				return [...trimmed, location.pathname];
			}
			return trimmed;
		});
		setCurrentIndex((prev) => {
			// Only increment if the location is new
			if (historyStack[prev] !== location.pathname) {
				return prev + 1;
			}
			return prev;
		});
	}, [location.pathname]);

	const canGoBack = currentIndex > 0;
	const canGoForward = currentIndex < historyStack.length - 1;

	const handleGoBack = () => {
		if (canGoBack) {
			const newIndex = currentIndex - 1;
			setCurrentIndex(newIndex);
			navigate(historyStack[newIndex]);
		}
	};

	const handleGoForward = () => {
		if (canGoForward) {
			const newIndex = currentIndex + 1;
			setCurrentIndex(newIndex);
			navigate(historyStack[newIndex]);
		}
	};

	return (
		<div className="bg-app text-ink relative h-full">
			{/* Top Bar */}
			<div
				data-tauri-drag-region
				className="top-bar-blur border-app-line bg-app/85 absolute inset-x-0 top-0 z-20 flex h-12 items-center gap-3 border-b px-3"
				style={{paddingLeft: isMacOS ? 92 : 12}}
			>
				{/* Back/Forward Navigation Buttons */}
				<CircleButtonGroup data-tauri-drag-region>
					<CircleButton
						icon={ArrowLeft}
						onClick={handleGoBack}
						disabled={!canGoBack}
						title="Go back"
					/>
					<CircleButton
						icon={ArrowRight}
						onClick={handleGoForward}
						disabled={!canGoForward}
						title="Go forward"
					/>
				</CircleButtonGroup>
				<Popover.Root open={agentSelector.open} onOpenChange={agentSelector.setOpen}>
					<Popover.Trigger asChild>
						<SelectPill variant="sidebar">
							{currentAgent?.name ?? 'Agent'}
						</SelectPill>
					</Popover.Trigger>
					<Popover.Content align="start" sideOffset={8} className="min-w-[180px] p-2">
						<OptionList>
							{agents.map((agent) => (
								<OptionListItem
									key={agent.id}
									selected={agent.id === selectedAgent}
									onClick={() => {
										setSelectedAgent(agent.id);
										agentSelector.setOpen(false);
									}}
								>
									<div>
										<div className="font-medium">{agent.name}</div>
										<div className="text-ink-dull text-xs">{agent.detail}</div>
									</div>
								</OptionListItem>
							))}
						</OptionList>
					</Popover.Content>
				</Popover.Root>

				<div className="flex-grow" />

				<div className="flex items-center gap-2" data-tauri-drag-region>
					<SearchBar
						value={search}
						onChange={setSearch}
						placeholder="Search"
						className="w-64"
					/>
					<Button
						onClick={navigateToChat}
						variant="accent"
						size="xs"
						className="rounded-full"
					>
						New chat
					</Button>
				</div>
			</div>

			{/* Main Content Area */}
			<div className="flex h-full pt-12">
				{/* Sidebar */}
				<aside className="border-app-line bg-sidebar flex w-[280px] shrink-0 flex-col border-r">
					<nav className="space-y-1 px-3 py-3">
						{primaryItems.map((item) => {
							const Icon = item.icon;
							const isActive =
								item.label === 'Chat'
									? isChatActive &&
										!location.pathname.includes(
											'/conversation/'
										)
									: location.pathname === item.path;
							return (
								<button
									key={item.label}
									onClick={() => navigate(item.path)}
									className={`focus:ring-accent flex w-full flex-row items-center gap-0.5 truncate rounded-lg px-2 py-1.5 text-left text-sm font-medium tracking-wide outline-none ring-inset ring-transparent transition-colors focus:ring-1 ${
										isActive
											? 'bg-sidebar-selected/40 text-sidebar-ink'
											: 'text-sidebar-inkDull hover:text-sidebar-ink'
									}`}
								>
									<Icon
										className="mr-2 size-4"
										weight={isActive ? 'fill' : 'bold'}
									/>
									<span className="truncate">
										{item.label}
									</span>
								</button>
							);
						})}
					</nav>

					<div className="flex-1 overflow-y-auto px-3 pb-4">
						<section className="mb-5">
							<div className="mb-2 flex items-center justify-between px-2">
								<div className="text-sidebar-inkDull text-[11px] font-semibold uppercase tracking-[0.16em]">
									Projects
								</div>
								<DotsThree className="text-sidebar-inkDull size-4" />
							</div>
							<div className="space-y-1">
								{projects.map((project) => (
									<button
										key={project.name}
										className="hover:bg-sidebar-box flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors"
									>
										<img
											src={project.ball}
											alt=""
											className="size-7 shrink-0 object-contain"
											draggable={false}
										/>
										<div>
											<div className="text-sidebar-ink text-sm font-medium">
												{project.name}
											</div>
											<div className="text-sidebar-inkDull text-xs">
												{project.detail}
											</div>
										</div>
									</button>
								))}
							</div>
						</section>

						<section>
							<div className="mb-2 flex items-center justify-between px-2">
								<div className="text-sidebar-inkDull text-[11px] font-semibold uppercase tracking-[0.16em]">
									History
								</div>
								<ClockCounterClockwise className="text-sidebar-inkDull size-4" />
							</div>
							<div className="space-y-1">
								<SidebarHistory />
							</div>
						</section>
					</div>
				</aside>

				{/* Main Content */}
				<main className="bg-app relative flex min-w-0 flex-1 px-6">
					<div
						aria-hidden="true"
						className="pointer-events-none absolute inset-0 z-0 opacity-100"
						style={{
							backgroundImage:
								'linear-gradient(to right, color-mix(in srgb, var(--color-app-line) 45%, transparent) 1px, transparent 1px), linear-gradient(to bottom, color-mix(in srgb, var(--color-app-line) 45%, transparent) 1px, transparent 1px)',
							backgroundSize: '28px 28px',
							maskImage:
								'linear-gradient(to bottom, rgba(0,0,0,0.42), rgba(0,0,0,0.08))',
							WebkitMaskImage:
								'linear-gradient(to bottom, rgba(0,0,0,0.42), rgba(0,0,0,0.08))'
						}}
					/>
					<div
						aria-hidden="true"
						className="pointer-events-none absolute inset-0 z-0"
						style={{
							background:
								'radial-gradient(circle at top, color-mix(in srgb, var(--color-accent) 8%, transparent), transparent 42%)'
						}}
					/>

					<div className="relative z-10 flex h-full w-full justify-center">
						<Outlet />
					</div>
				</main>
			</div>

			<ReactQueryDevtools
				initialIsOpen={false}
				buttonPosition="bottom-left"
			/>
		</div>
	);
}
