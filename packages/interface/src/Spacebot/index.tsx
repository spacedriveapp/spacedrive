import {
	Atom,
	Brain,
	CalendarDots,
	CaretDown,
	ChatCircleDots,
	Checks,
	ClockCounterClockwise,
	DotsThree
} from '@phosphor-icons/react';
import {Ball, BallBlue} from '@sd/assets/images';
import {Popover, SearchBar, usePopover} from '@sd/ui';
import {
	apiClient,
	getEventsUrl,
	setServerUrl,
	type InboundMessageEvent,
	type OutboundMessageDeltaEvent,
	type OutboundMessageEvent,
	type TypingStateEvent,
	type WebChatConversationResponse,
	type WebChatConversationSummary
} from '@spacebot/api-client';
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query';
import {useEffect, useState} from 'react';
import {usePlatform} from '../contexts/PlatformContext';
import {ChatComposer} from './ChatComposer';
import {ConversationScreen} from './ConversationScreen';
import {useSpacebotEventSource} from './useSpacebotEventSource';

const isMacOS =
	typeof navigator !== 'undefined' &&
	(navigator.platform.toLowerCase().includes('mac') ||
		navigator.userAgent.includes('Mac'));

const primaryItems = [
	{icon: ChatCircleDots, label: 'Chat'},
	{icon: Checks, label: 'Tasks'},
	{icon: Brain, label: 'Memories'},
	{icon: Atom, label: 'Autonomy'},
	{icon: CalendarDots, label: 'Schedule'}
];

const projects = [
	{name: 'Spacedrive v3', detail: 'Main workspace', ball: BallBlue},
	{name: 'Spacebot Runtime', detail: 'Remote control plane', ball: Ball},
	{name: 'Hosted Platform', detail: 'Deploy and observe', ball: Ball}
];

const agents = [
	{id: 'main', name: 'James', detail: 'Founder mode'},
	{id: 'operations', name: 'Operations', detail: 'Scheduling and triage'},
	{id: 'builder', name: 'Builder', detail: 'Code and tooling'}
];

const projectOptions = ['Spacedrive v3', 'Spacebot Runtime', 'Hosted Platform'];
const modelOptions = ['Claude 3.7 Sonnet', 'GPT-5', 'Qwen 2.5 72B'];

export function Spacebot() {
	const platform = usePlatform();
	const queryClient = useQueryClient();
	useEffect(() => {
		setServerUrl('http://127.0.0.1:19898');
	}, []);

	const [search, setSearch] = useState('');
	const [activeTab, setActiveTab] = useState('Chat');
	const [selectedAgent, setSelectedAgent] = useState('main');
	const [selectedConversationId, setSelectedConversationId] = useState<
		string | null
	>(null);
	const [selectedProject, setSelectedProject] = useState(
		projectOptions[0] ?? ''
	);
	const [selectedModel, setSelectedModel] = useState(modelOptions[0] ?? '');
	const [draft, setDraft] = useState('');
	const [isTyping, setIsTyping] = useState(false);
	const [streamingAssistantText, setStreamingAssistantText] = useState('');

	const agentSelector = usePopover();
	const composerProjectSelector = usePopover();
	const composerModelSelector = usePopover();

	const currentAgent =
		agents.find((agent) => agent.id === selectedAgent) ?? agents[0];

	useEffect(() => {
		if (platform.applyMacOSStyling) {
			platform.applyMacOSStyling().catch((error) => {
				console.warn('Failed to apply macOS styling:', error);
			});
		}
	}, [platform]);

	const conversationsQuery = useQuery({
		queryKey: ['spacebot', 'conversations', selectedAgent],
		queryFn: () =>
			apiClient.listWebchatConversations(selectedAgent, false, 100),
		refetchInterval: 4000
	});

	const selectedConversation =
		conversationsQuery.data?.conversations.find(
			(conversation) => conversation.id === selectedConversationId
		) ?? null;
	const isConversationView =
		activeTab === 'Chat' && selectedConversationId !== null;

	const historyQuery = useQuery({
		queryKey: [
			'spacebot',
			'webchat-history',
			selectedAgent,
			selectedConversationId
		],
		queryFn: () =>
			apiClient.webchatHistory(
				selectedAgent,
				selectedConversationId!,
				200
			),
		enabled: Boolean(selectedConversationId && activeTab === 'Chat'),
		refetchInterval: false
	});

	useEffect(() => {
		setSelectedConversationId(null);
		setIsTyping(false);
		setStreamingAssistantText('');
	}, [selectedAgent]);

	useEffect(() => {
		if (activeTab !== 'Chat') return;
		const conversations = conversationsQuery.data?.conversations ?? [];
		if (conversations.length === 0) {
			setSelectedConversationId(null);
			return;
		}

		setSelectedConversationId((current) => {
			if (
				current &&
				conversations.some(
					(conversation) => conversation.id === current
				)
			) {
				return current;
			}
			return conversations[0]?.id ?? null;
		});
	}, [activeTab, conversationsQuery.data]);

	useEffect(() => {
		setIsTyping(false);
		setStreamingAssistantText('');
	}, [selectedConversationId]);

	const createConversation = useMutation({
		mutationFn: (title?: string | null) =>
			apiClient.createWebchatConversation({
				agentId: selectedAgent,
				title
			}),
		onSuccess: async (response: WebChatConversationResponse) => {
			setSelectedConversationId(response.conversation.id);
			await queryClient.invalidateQueries({
				queryKey: ['spacebot', 'conversations', selectedAgent]
			});
		}
	});

	const sendMessage = useMutation({
		mutationFn: async (message: string) => {
			let conversationId = selectedConversationId;
			if (!conversationId) {
				const response = await createConversation.mutateAsync(null);
				conversationId = response.conversation.id;
			}

			await apiClient.webchatSend({
				agentId: selectedAgent,
				sessionId: conversationId,
				senderName: currentAgent?.name ?? 'user',
				message
			});

			return conversationId;
		},
		onSuccess: async (conversationId) => {
			setDraft('');
			setSelectedConversationId(conversationId);
			await Promise.all([
				queryClient.invalidateQueries({
					queryKey: ['spacebot', 'conversations', selectedAgent]
				}),
				queryClient.invalidateQueries({
					queryKey: [
						'spacebot',
						'webchat-history',
						selectedAgent,
						conversationId
					]
				})
			]);
		}
	});

	useSpacebotEventSource(getEventsUrl(), {
		enabled: activeTab === 'Chat',
		onReconnect: () => {
			void queryClient.invalidateQueries({
				queryKey: ['spacebot', 'conversations', selectedAgent]
			});
			if (selectedConversationId) {
				void queryClient.invalidateQueries({
					queryKey: [
						'spacebot',
						'webchat-history',
						selectedAgent,
						selectedConversationId
					]
				});
			}
		},
		handlers: {
			typing_state: (payload) => {
				const event = payload as TypingStateEvent;
				if (
					event.agent_id !== selectedAgent ||
					event.channel_id !== selectedConversationId
				) {
					return;
				}

				setIsTyping(event.is_typing);
				if (!event.is_typing) {
					setStreamingAssistantText('');
				}
			},
			outbound_message_delta: (payload) => {
				const event = payload as OutboundMessageDeltaEvent;
				if (
					event.agent_id !== selectedAgent ||
					event.channel_id !== selectedConversationId
				) {
					return;
				}

				setIsTyping(true);
				setStreamingAssistantText(event.aggregated_text);
			},
			outbound_message: (payload) => {
				const event = payload as OutboundMessageEvent;
				if (
					event.agent_id !== selectedAgent ||
					event.channel_id !== selectedConversationId
				) {
					return;
				}

				setIsTyping(false);
				setStreamingAssistantText('');
				void Promise.all([
					queryClient.invalidateQueries({
						queryKey: ['spacebot', 'conversations', selectedAgent]
					}),
					queryClient.invalidateQueries({
						queryKey: [
							'spacebot',
							'webchat-history',
							selectedAgent,
							selectedConversationId
						]
					})
				]);
			},
			inbound_message: (payload) => {
				const event = payload as InboundMessageEvent;
				if (
					event.agent_id !== selectedAgent ||
					event.channel_id !== selectedConversationId
				) {
					return;
				}

				void Promise.all([
					queryClient.invalidateQueries({
						queryKey: ['spacebot', 'conversations', selectedAgent]
					}),
					queryClient.invalidateQueries({
						queryKey: [
							'spacebot',
							'webchat-history',
							selectedAgent,
							selectedConversationId
						]
					})
				]);
			}
		}
	});

	function openVoiceOverlay() {
		if (!platform.showWindow) return;
		platform.showWindow({type: 'VoiceOverlay'}).catch((error) => {
			console.warn('Failed to open voice overlay:', error);
		});
	}

	async function handleSendMessage() {
		const message = draft.trim();
		if (!message || sendMessage.isPending) return;
		await sendMessage.mutateAsync(message);
	}

	function renderSidebarHistory() {
		if (conversationsQuery.isLoading) {
			return (
				<div className="text-sidebar-inkDull px-3 py-2 text-xs">
					Loading conversations...
				</div>
			);
		}

		if (conversationsQuery.isError) {
			return (
				<div className="text-sidebar-inkDull px-3 py-2 text-xs">
					Could not load conversations.
				</div>
			);
		}

		const conversations = conversationsQuery.data?.conversations ?? [];
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

		return filtered.map((conversation) => {
			const isActive = selectedConversationId === conversation.id;
			return (
				<button
					key={conversation.id}
					onClick={() => {
						setSelectedConversationId(conversation.id);
					}}
					className={`flex w-full items-start rounded-lg px-3 py-2 text-left transition-colors ${
						isActive
							? 'bg-sidebar-selected/40'
							: 'hover:bg-sidebar-box'
					}`}
				>
					<div>
						<div className="text-sidebar-ink text-sm font-medium">
							{conversation.title}
						</div>
						<div className="text-sidebar-inkDull line-clamp-2 text-xs">
							{conversation.last_message_preview ??
								'No messages yet'}
						</div>
					</div>
				</button>
			);
		});
	}

	function renderMainContent() {
		if (activeTab !== 'Chat') {
			return (
				<div className="border-app-line bg-app-box/90 w-full max-w-3xl rounded-[28px] border p-6 text-left shadow-[0_30px_80px_rgba(0,0,0,0.25)] backdrop-blur-xl">
					<h1 className="text-ink text-3xl font-semibold">
						{activeTab}
					</h1>
					<p className="text-ink-dull mt-2 text-sm">
						Dedicated {activeTab.toLowerCase()} UI comes next.
					</p>
				</div>
			);
		}

		return (
			<ConversationScreen
				conversation={isConversationView ? selectedConversation : null}
				messages={historyQuery.data ?? []}
				isTyping={isTyping}
				streamingAssistantText={streamingAssistantText}
				draft={draft}
				onDraftChange={setDraft}
				onSend={() => void handleSendMessage()}
				onOpenVoiceOverlay={openVoiceOverlay}
				selectedProject={selectedProject}
				selectedModel={selectedModel}
				projectOptions={projectOptions}
				modelOptions={modelOptions}
				onSelectProject={setSelectedProject}
				onSelectModel={setSelectedModel}
				projectSelector={composerProjectSelector}
				modelSelector={composerModelSelector}
				isSending={
					sendMessage.isPending || createConversation.isPending
				}
			/>
		);
	}

	return (
		<div className="bg-app text-ink relative h-full overflow-hidden">
			<div
				data-tauri-drag-region
				className="top-bar-blur border-app-line bg-app/85 absolute inset-x-0 top-0 z-20 flex h-12 items-center justify-between border-b px-3"
				style={{paddingLeft: isMacOS ? 92 : 12}}
			>
				<div className="flex items-center gap-3" data-tauri-drag-region>
					<div className="w-[156px]" data-tauri-drag-region>
						<Popover
							popover={agentSelector}
							trigger={
								<button className="border-sidebar-line/30 bg-sidebar-box/20 text-sidebar-inkDull hover:bg-sidebar-box/30 hover:text-sidebar-ink flex h-8 w-full items-center gap-2 rounded-full border px-3 text-left text-xs font-medium backdrop-blur-xl transition-all active:scale-95">
									<span className="flex-1 truncate text-left">
										{currentAgent?.name ?? 'Agent'}
									</span>
									<CaretDown
										className="size-3"
										weight="bold"
									/>
								</button>
							}
							align="start"
							sideOffset={8}
							className="min-w-[180px] p-2"
						>
							<div className="space-y-1">
								{agents.map((agent) => (
									<button
										key={agent.id}
										onClick={() => {
											setSelectedAgent(agent.id);
											agentSelector.setOpen(false);
										}}
										className="text-ink hover:bg-app-selected w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm transition-colors"
									>
										<div>
											<div className="font-medium">
												{agent.name}
											</div>
											<div className="text-ink-dull text-xs">
												{agent.detail}
											</div>
										</div>
									</button>
								))}
							</div>
						</Popover>
					</div>
				</div>

				<div className="flex items-center gap-2" data-tauri-drag-region>
					<SearchBar
						value={search}
						onChange={setSearch}
						placeholder="Search"
						className="w-64"
					/>
					<button
						onClick={() => {
							setActiveTab('Chat');
							setSelectedConversationId(null);
							setDraft('');
							setIsTyping(false);
							setStreamingAssistantText('');
						}}
						className="border-app-line bg-accent hover:bg-accent-faint rounded-full border px-3 py-1.5 text-xs font-medium text-white transition-colors"
					>
						New chat
					</button>
				</div>
			</div>

			<div className="flex h-full pt-12">
				<aside className="border-app-line bg-sidebar flex w-[280px] shrink-0 flex-col border-r">
					<nav className="space-y-1 px-3 py-3">
						{primaryItems.map((item) => {
							const Icon = item.icon;
							const isActive =
								item.label === 'Chat'
									? activeTab === 'Chat' && !isConversationView
									: activeTab === item.label;
							return (
								<button
									key={item.label}
									onClick={() => {
										setActiveTab(item.label);
										if (item.label === 'Chat') {
											setSelectedConversationId(null);
											setIsTyping(false);
											setStreamingAssistantText('');
										}
									}}
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
								{renderSidebarHistory()}
							</div>
						</section>
					</div>
				</aside>

				<main className="bg-app relative flex min-w-0 flex-1 items-center justify-center overflow-hidden p-6">
					<div
						aria-hidden="true"
						className="pointer-events-none absolute inset-0 opacity-100"
						style={{
							backgroundImage:
								'linear-gradient(to right, hsla(var(--color-app-line), 0.45) 1px, transparent 1px), linear-gradient(to bottom, hsla(var(--color-app-line), 0.45) 1px, transparent 1px)',
							backgroundSize: '28px 28px',
							maskImage:
								'linear-gradient(to bottom, rgba(0,0,0,0.42), rgba(0,0,0,0.08))',
							WebkitMaskImage:
								'linear-gradient(to bottom, rgba(0,0,0,0.42), rgba(0,0,0,0.08))'
						}}
					/>
					<div
						aria-hidden="true"
						className="pointer-events-none absolute inset-0"
						style={{
							background:
								'radial-gradient(circle at top, hsla(var(--color-accent), 0.08), transparent 42%)'
						}}
					/>

					{renderMainContent()}
				</main>
			</div>
		</div>
	);
}
