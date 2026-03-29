import {
	Brain,
	CalendarDots,
	ChatCircleDots,
	Checks,
	DotsThree,
	MoonStars
} from '@phosphor-icons/react';
import {Ball, BallBlue} from '@sd/assets/images';
import {
	apiClient,
	getEventsUrl,
	setServerUrl,
	type InboundMessageEvent,
	type OutboundMessageDeltaEvent,
	type OutboundMessageEvent,
	type PortalConversationResponse,
	type PortalConversationSummary,
	type TypingStateEvent
} from '@spacebot/api-client';
import {SearchBar, usePopover} from '@spaceui/primitives';
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query';
import {
	createContext,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useState,
	type ReactNode
} from 'react';
import {useNavigate, useParams} from 'react-router-dom';
import {usePlatform} from '../contexts/PlatformContext';
import {useSpacebotEventSource} from './useSpacebotEventSource';

export const primaryItems = [
	{icon: ChatCircleDots, label: 'Chat', path: '/spacebot/chat'},
	{icon: Checks, label: 'Tasks', path: '/spacebot/tasks'},
	{icon: Brain, label: 'Memories', path: '/spacebot/memories'},
	{icon: MoonStars, label: 'Dream', path: '/spacebot/autonomy'},
	{icon: CalendarDots, label: 'Schedule', path: '/spacebot/schedule'}
];

export const projects = [
	{name: 'Spacedrive', detail: 'Main workspace', ball: BallBlue},
	{name: 'Spacebot Runtime', detail: 'Remote control plane', ball: Ball},
	{name: 'Hosted Platform', detail: 'Deploy and observe', ball: Ball}
];

export const agents = [
	{id: 'main', name: 'Star', detail: 'Spacedrive COO'},
	{id: 'operations', name: 'Operations', detail: 'Scheduling and triage'},
	{id: 'builder', name: 'Builder', detail: 'Code and tooling'}
];

export const projectOptions = [
	'Spacedrive v3',
	'Spacebot Runtime',
	'Hosted Platform'
];
export const models = [
	{
		id: 'claude-3.7-sonnet',
		name: 'Claude 3.7 Sonnet',
		provider: 'Anthropic',
		context_window: 200000
	},
	{id: 'gpt-5', name: 'GPT-5', provider: 'OpenAI', context_window: 128000},
	{
		id: 'qwen-2.5-72b',
		name: 'Qwen 2.5 72B',
		provider: 'Qwen',
		context_window: 32000
	}
];

interface SpacebotContextType {
	// Navigation state
	search: string;
	setSearch: (value: string) => void;
	selectedAgent: string;
	setSelectedAgent: (value: string) => void;
	activeTab: string;

	// Agent data
	currentAgent: (typeof agents)[number];
	agentSelector: ReturnType<typeof usePopover>;

	// Composer state
	selectedProject: string;
	setSelectedProject: (value: string) => void;
	selectedModel: string;
	setSelectedModel: (value: string) => void;
	projectOptions: string[];
	models: typeof models;
	composerProjectSelector: ReturnType<typeof usePopover>;

	// Conversation state
	draft: string;
	setDraft: (value: string) => void;
	isTyping: boolean;
	streamingAssistantText: string;
	conversations: PortalConversationSummary[];
	conversationsLoading: boolean;
	conversationsError: Error | null;

	// Actions
	handleSendMessage: () => Promise<void>;
	isSending: boolean;
	createConversation: (
		title?: string | null
	) => Promise<PortalConversationResponse>;
	getConversationById: (id: string) => PortalConversationSummary | undefined;
	getConversationMessages: (id: string) => PortalHistoryItem[];
	openVoiceOverlay: () => void;

	// Navigation
	navigateToChat: () => void;
	navigateToConversation: (conversationId: string) => void;
}

interface PortalHistoryItem {
	role: 'user' | 'assistant';
	content: string;
	timestamp: string;
}

const SpacebotContext = createContext<SpacebotContextType | null>(null);

export function useSpacebot() {
	const context = useContext(SpacebotContext);
	if (!context) {
		throw new Error('useSpacebot must be used within SpacebotProvider');
	}
	return context;
}

interface SpacebotProviderProps {
	children: ReactNode;
}

export const isMacOS =
	typeof navigator !== 'undefined' &&
	(navigator.platform.toLowerCase().includes('mac') ||
		navigator.userAgent.includes('Mac'));

export function SpacebotProvider({children}: SpacebotProviderProps) {
	const platform = usePlatform();
	const queryClient = useQueryClient();
	const navigate = useNavigate();
	const params = useParams();

	useEffect(() => {
		setServerUrl('http://127.0.0.1:19898');
	}, []);

	useEffect(() => {
		if (platform.applyMacOSStyling) {
			platform.applyMacOSStyling().catch((error) => {
				console.warn('Failed to apply macOS styling:', error);
			});
		}
	}, [platform]);

	// Navigation state
	const [search, setSearch] = useState('');
	const [selectedAgent, setSelectedAgent] = useState('main');
	const [activeTab, setActiveTab] = useState('Chat');

	// Composer state
	const [selectedProject, setSelectedProject] = useState(
		projectOptions[0] ?? ''
	);
	const [selectedModel, setSelectedModel] = useState(models[0]?.id ?? '');

	// Conversation state
	const [draft, setDraft] = useState('');
	const [isTyping, setIsTyping] = useState(false);
	const [streamingAssistantText, setStreamingAssistantText] = useState('');
	const [conversationMessages, setConversationMessages] = useState<
		Map<string, PortalHistoryItem[]>
	>(new Map());

	const agentSelector = usePopover();
	const composerProjectSelector = usePopover();

	const currentAgent = useMemo(
		() => agents.find((agent) => agent.id === selectedAgent) ?? agents[0],
		[selectedAgent]
	);

	// Reset state when agent changes
	useEffect(() => {
		setIsTyping(false);
		setStreamingAssistantText('');
	}, [selectedAgent]);

	// Conversations query
	const conversationsQuery = useQuery({
		queryKey: ['spacebot', 'conversations', selectedAgent],
		queryFn: () =>
			apiClient.listPortalConversations(selectedAgent, false, 100),
		refetchInterval: 4000
	});

	const conversations = conversationsQuery.data?.conversations ?? [];

	// Conversation messages query - fetch when viewing a conversation
	// With splat route "conversation/*", the ID is in params["*"]
	const conversationId = params['*']
		? decodeURIComponent(params['*'])
		: undefined;
	const historyQuery = useQuery({
		queryKey: ['spacebot', 'portal-history', selectedAgent, conversationId],
		queryFn: () =>
			apiClient.portalHistory(selectedAgent, conversationId!, 200),
		enabled: Boolean(conversationId),
		refetchInterval: false
	});

	// Update conversation messages cache
	useEffect(() => {
		if (historyQuery.data && conversationId) {
			setConversationMessages((prev) => {
				const next = new Map(prev);
				next.set(
					conversationId,
					historyQuery.data as unknown as PortalHistoryItem[]
				);
				return next;
			});
		}
	}, [historyQuery.data, conversationId]);

	// Create conversation mutation
	const createConversationMutation = useMutation({
		mutationFn: (title?: string | null) =>
			apiClient.createPortalConversation({
				agentId: selectedAgent,
				title
			}),
		onSuccess: async (response: PortalConversationResponse) => {
			navigateToConversation(response.conversation.id);
			await queryClient.invalidateQueries({
				queryKey: ['spacebot', 'conversations', selectedAgent]
			});
		}
	});

	// Send message mutation
	const sendMessageMutation = useMutation({
		mutationFn: async (message: string) => {
			let targetConversationId = conversationId;
			if (!targetConversationId) {
				const response =
					await createConversationMutation.mutateAsync(null);
				targetConversationId = response.conversation.id;
			}

			await apiClient.portalSend({
				agentId: selectedAgent,
				sessionId: targetConversationId!,
				senderName: currentAgent?.name ?? 'user',
				message
			});

			return targetConversationId;
		},
		onSuccess: async (targetConversationId) => {
			setDraft('');
			if (targetConversationId) {
				navigateToConversation(targetConversationId);
			}
			await Promise.all([
				queryClient.invalidateQueries({
					queryKey: ['spacebot', 'conversations', selectedAgent]
				}),
				queryClient.invalidateQueries({
					queryKey: [
						'spacebot',
						'portal-history',
						selectedAgent,
						targetConversationId
					]
				})
			]);
		}
	});

	// SSE event source
	useSpacebotEventSource(getEventsUrl(), {
		enabled: activeTab === 'Chat',
		onReconnect: () => {
			void queryClient.invalidateQueries({
				queryKey: ['spacebot', 'conversations', selectedAgent]
			});
			if (conversationId) {
				void Promise.all([
					queryClient.invalidateQueries({
						queryKey: [
							'spacebot',
							'portal-history',
							selectedAgent,
							conversationId
						]
					}),
					queryClient.invalidateQueries({
						queryKey: [
							'spacebot',
							'channel-timeline',
							conversationId
						]
					})
				]);
			}
		},
		handlers: {
			typing_state: (payload) => {
				const event = payload as TypingStateEvent;
				if (
					event.agent_id !== selectedAgent ||
					event.channel_id !== conversationId
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
					event.channel_id !== conversationId
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
					event.channel_id !== conversationId
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
							'portal-history',
							selectedAgent,
							conversationId
						]
					}),
					queryClient.invalidateQueries({
						queryKey: [
							'spacebot',
							'channel-timeline',
							conversationId
						]
					})
				]);
			},
			inbound_message: (payload) => {
				const event = payload as InboundMessageEvent;
				if (
					event.agent_id !== selectedAgent ||
					event.channel_id !== conversationId
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
							'portal-history',
							selectedAgent,
							conversationId
						]
					}),
					queryClient.invalidateQueries({
						queryKey: [
							'spacebot',
							'channel-timeline',
							conversationId
						]
					})
				]);
			}
		}
	});

	// Actions
	const handleSendMessage = useCallback(async () => {
		const message = draft.trim();
		if (!message || sendMessageMutation.isPending) return;
		await sendMessageMutation.mutateAsync(message);
	}, [draft, sendMessageMutation]);

	const createConversation = useCallback(
		async (title?: string | null) => {
			return createConversationMutation.mutateAsync(title);
		},
		[createConversationMutation]
	);

	const getConversationById = useCallback(
		(id: string) => conversations.find((c) => c.id === id),
		[conversations]
	);

	const getConversationMessages = useCallback(
		(id: string) => conversationMessages.get(id) ?? [],
		[conversationMessages]
	);

	const openVoiceOverlay = useCallback(() => {
		if (!platform.showWindow) return;
		platform.showWindow({type: 'VoiceOverlay'}).catch((error) => {
			console.warn('Failed to open voice overlay:', error);
		});
	}, [platform]);

	const navigateToChat = useCallback(() => {
		setActiveTab('Chat');
		setIsTyping(false);
		setStreamingAssistantText('');
		navigate('/spacebot/chat');
	}, [navigate]);

	const navigateToConversation = useCallback(
		(conversationId: string) => {
			setActiveTab('Chat');
			setIsTyping(false);
			setStreamingAssistantText('');
			navigate(`/spacebot/chat/conversation/${conversationId}`);
		},
		[navigate]
	);

	const value = useMemo(
		() => ({
			search,
			setSearch,
			selectedAgent,
			setSelectedAgent,
			activeTab,
			currentAgent,
			agentSelector,
			selectedProject,
			setSelectedProject,
			selectedModel,
			setSelectedModel,
			projectOptions,
			models,
			composerProjectSelector,
			draft,
			setDraft,
			isTyping,
			streamingAssistantText,
			conversations,
			conversationsLoading: conversationsQuery.isLoading,
			conversationsError: conversationsQuery.error ?? null,
			handleSendMessage,
			isSending:
				sendMessageMutation.isPending ||
				createConversationMutation.isPending,
			createConversation,
			getConversationById,
			getConversationMessages,
			openVoiceOverlay,
			navigateToChat,
			navigateToConversation
		}),
		[
			search,
			selectedAgent,
			activeTab,
			currentAgent,
			agentSelector,
			selectedProject,
			selectedModel,
			projectOptions,
			models,
			composerProjectSelector,
			draft,
			isTyping,
			streamingAssistantText,
			conversations,
			conversationsQuery.isLoading,
			conversationsQuery.error,
			handleSendMessage,
			sendMessageMutation.isPending,
			createConversationMutation.isPending,
			createConversation,
			getConversationById,
			getConversationMessages,
			openVoiceOverlay,
			navigateToChat,
			navigateToConversation
		]
	);

	return (
		<SpacebotContext.Provider value={value}>
			{children}
		</SpacebotContext.Provider>
	);
}
