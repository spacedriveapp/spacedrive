import {Copy} from '@phosphor-icons/react';
import {TopBarButton} from '@sd/ui';
import {
	apiClient,
	type TimelineItem,
	type WorkerListItem
} from '@spacebot/api-client';
import type {
	WebChatConversationSummary,
	WebChatHistoryMessage
} from '@spacebot/api-client';
import {useQuery} from '@tanstack/react-query';
import {useVirtualizer} from '@tanstack/react-virtual';
import {useEffect, useMemo, useRef} from 'react';
import {ChatComposer} from './ChatComposer';
import {EmptyChatHero} from './EmptyChatHero';
import {InlineWorkerCard} from './InlineWorkerCard';
import {Markdown} from './Markdown';

interface ConversationScreenProps {
	agentId: string;
	conversation: WebChatConversationSummary | null;
	messages: WebChatHistoryMessage[];
	isTyping: boolean;
	streamingAssistantText: string;
	draft: string;
	onDraftChange(value: string): void;
	onSend(): void;
	onOpenVoiceOverlay(): void;
	selectedProject: string;
	selectedModel: string;
	projectOptions: string[];
	modelOptions: string[];
	onSelectProject(project: string): void;
	onSelectModel(model: string): void;
	projectSelector: ReturnType<typeof import('@sd/ui').usePopover>;
	modelSelector: ReturnType<typeof import('@sd/ui').usePopover>;
	isSending?: boolean;
}

function MessageBubble({
	content,
	isUser,
	isStreaming = false,
	onCopy
}: {
	content: string;
	isUser: boolean;
	isStreaming?: boolean;
	onCopy?: (content: string) => void;
}) {
	return (
		<div className={`group flex flex-col py-2 ${isUser ? 'items-end' : 'items-start'}`}>
			<div
				className={`max-w-[80%] rounded-2xl px-4 py-3 text-[15px] leading-7 ${
					isUser ? 'bg-accent text-white' : 'border-app-line bg-app text-ink border'
				}`}
			>
				{isUser ? (
					<div className="whitespace-pre-wrap break-words">{content}</div>
				) : (
					<Markdown className="break-words">{content}</Markdown>
				)}
			</div>
			{!isUser && onCopy ? (
				<div className="mt-2 flex opacity-0 transition-opacity duration-150 group-hover:opacity-100">
					<TopBarButton
						icon={Copy}
						onClick={() => onCopy(content)}
						title={isStreaming ? 'Copy streaming message' : 'Copy message'}
						className="h-7 w-7"
					/>
				</div>
			) : null}
		</div>
	);
}

export function ConversationScreen({
	agentId,
	conversation,
	messages: _messages,
	isTyping,
	streamingAssistantText,
	draft,
	onDraftChange,
	onSend,
	onOpenVoiceOverlay,
	selectedProject,
	selectedModel,
	projectOptions,
	modelOptions,
	onSelectProject,
	onSelectModel,
	projectSelector,
	modelSelector,
	isSending = false
}: ConversationScreenProps) {
	const scrollRef = useRef<HTMLDivElement>(null);
	const previousTimelineLengthRef = useRef(0);
	const timelineQuery = useQuery({
		queryKey: ['spacebot', 'channel-timeline', conversation?.id],
		queryFn: () => apiClient.channelMessages(conversation!.id, 200),
		enabled: Boolean(conversation?.id),
		refetchInterval: 2000
	});
	const workersQuery = useQuery({
		queryKey: [
			'spacebot',
			'conversation-workers',
			agentId,
			conversation?.id
		],
		queryFn: () => apiClient.listWorkers({agentId, limit: 20}),
		enabled: Boolean(conversation?.id),
		refetchInterval: 2000
	});

	const timelineItems = timelineQuery.data?.items ?? [];
	const builtInWorkers = (workersQuery.data?.workers ?? []).filter(
		(worker: WorkerListItem) => {
			if (!conversation?.id) return false;
			if (worker.channel_id !== conversation.id) return false;
			if (worker.worker_type === 'opencode') return false;
			return true;
		}
	);
	const builtInWorkerIds = new Set(builtInWorkers.map((worker) => worker.id));
	const visibleTimelineItems = timelineItems.filter((item: TimelineItem) => {
		if (item.type !== 'worker_run') return true;
		return builtInWorkerIds.has(item.id);
	});
	const hasStreamingBubble = streamingAssistantText.trim().length > 0;
	const timelineSignature = useMemo(
		() => visibleTimelineItems.map((item) => `${item.type}:${item.id}`).join('|'),
		[visibleTimelineItems]
	);

	const virtualizer = useVirtualizer({
		count: visibleTimelineItems.length,
		getScrollElement: () => scrollRef.current,
		estimateSize: (index) => {
			const item = visibleTimelineItems[index];
			if (!item) return 80;
			if (item.type === 'worker_run') return 96;
			if (item.type !== 'message') return 80;
			const base = item.role === 'user' ? 72 : 88;
			const lines = Math.ceil(
				item.content.length / (item.role === 'user' ? 42 : 56)
			);
			return Math.min(480, base + lines * 28);
		},
		overscan: 8
	});

	const copyMessage = async (content: string) => {
		await navigator.clipboard.writeText(content);
	};

	useEffect(() => {
		const element = scrollRef.current;
		if (!element) return;

		const previousLength = previousTimelineLengthRef.current;
		const currentLength = visibleTimelineItems.length;
		const distanceFromBottom =
			element.scrollHeight - element.scrollTop - element.clientHeight;
		const isNearBottom = distanceFromBottom < 160;
		const shouldAutoScroll =
			conversation?.id != null &&
			(currentLength > previousLength || Boolean(streamingAssistantText) || isTyping) &&
			(previousLength === 0 || isNearBottom);

		if (shouldAutoScroll) {
			requestAnimationFrame(() => {
				element.scrollTo({top: element.scrollHeight, behavior: 'auto'});
			});
		}

		previousTimelineLengthRef.current = currentLength;
	}, [timelineSignature, visibleTimelineItems.length, streamingAssistantText, isTyping, conversation?.id]);

	if (!conversation) {
		return (
			<div className="flex h-full w-full items-center justify-center py-10">
				<div className="w-full max-w-3xl">
					<EmptyChatHero />

					<ChatComposer
						draft={draft}
						onDraftChange={onDraftChange}
						onSend={onSend}
						onOpenVoiceOverlay={onOpenVoiceOverlay}
						selectedProject={selectedProject}
						selectedModel={selectedModel}
						projectOptions={projectOptions}
						modelOptions={modelOptions}
						onSelectProject={onSelectProject}
						onSelectModel={onSelectModel}
						projectSelector={projectSelector}
						modelSelector={modelSelector}
						isSending={isSending}
					/>
				</div>
			</div>
		);
	}

	return (
		<div className="relative flex h-full w-full max-w-4xl flex-col">
			{/* <div className="pointer-events-none absolute inset-x-0 top-0 z-10 px-6 py-4">
				<div className="text-ink text-lg font-semibold">
					{conversation.title}
				</div>
				<div className="text-ink-dull mt-1 text-xs uppercase tracking-[0.14em]">
					{conversation.message_count} messages
				</div>
			</div> */}

			<div
				ref={scrollRef}
				className="flex-1 space-y-4 overflow-y-auto px-6"
			>
				<div className="h-24 shrink-0" />
				{visibleTimelineItems.length > 0 ? (
					<div
						className="relative w-full"
						style={{height: virtualizer.getTotalSize()}}
					>
						{virtualizer.getVirtualItems().map((virtualRow) => {
							const item = visibleTimelineItems[virtualRow.index];
							if (!item) return null;

							return (
								<div
									key={item.id}
									data-index={virtualRow.index}
									ref={virtualizer.measureElement}
									className="absolute left-0 top-0 w-full"
									style={{
										transform: `translateY(${virtualRow.start}px)`
									}}
								>
									{item.type === 'worker_run' ? (
										<div className="py-2">
											<InlineWorkerCard
												agentId={agentId}
												worker={
													builtInWorkers.find((worker) => worker.id === item.id) ?? {
														id: item.id,
														task: item.task,
														status: item.status,
														worker_type: 'builtin',
														channel_id: conversation.id,
														channel_name: null,
														started_at: item.started_at,
														completed_at: item.completed_at,
														has_transcript: true,
														live_status: null,
														tool_calls: 0,
														opencode_port: null,
														opencode_session_id: null,
														directory: null,
														interactive: false,
														project_id: null,
														project_name: null
													}
												}
											/>
										</div>
									) : item.type === 'message' ? (
										(() => {
											return (
												<MessageBubble
													content={item.content}
													isUser={item.role === 'user'}
													onCopy={(content) => void copyMessage(content)}
												/>
											);
										})()
									) : null}
								</div>
							);
						})}
					</div>
				) : (
					<div className="text-ink-dull flex h-full min-h-[240px] items-center justify-center text-sm">
						Start the conversation here.
					</div>
				)}

				{hasStreamingBubble ? (
					<MessageBubble
						content={streamingAssistantText}
						isUser={false}
						isStreaming
						onCopy={(content) => void copyMessage(content)}
					/>
				) : null}

				{isTyping && !streamingAssistantText ? (
					<div className="flex justify-start">
						<div className="border-app-line bg-app text-ink-dull rounded-2xl border px-4 py-3 text-sm">
							Spacebot is typing...
						</div>
					</div>
				) : null}
				<div className="h-72 shrink-0" />
			</div>

			<div className="absolute inset-x-0 bottom-0 z-10 p-4">
				<ChatComposer
					draft={draft}
					onDraftChange={onDraftChange}
					onSend={onSend}
					onOpenVoiceOverlay={onOpenVoiceOverlay}
					selectedProject={selectedProject}
					selectedModel={selectedModel}
					projectOptions={projectOptions}
					modelOptions={modelOptions}
					onSelectProject={onSelectProject}
					onSelectModel={onSelectModel}
					projectSelector={projectSelector}
					modelSelector={modelSelector}
					showHeading={false}
					showOuterBox={false}
					isSending={isSending}
				/>
			</div>
		</div>
	);
}
