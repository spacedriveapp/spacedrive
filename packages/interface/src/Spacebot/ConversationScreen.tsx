import {Copy} from '@phosphor-icons/react';
import {TopBarButton} from '@sd/ui';
import type {
	WebChatConversationSummary,
	WebChatHistoryMessage
} from '@spacebot/api-client';
import {useEffect, useRef} from 'react';
import {ChatComposer} from './ChatComposer';

interface ConversationScreenProps {
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

export function ConversationScreen({
	conversation,
	messages,
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

	const copyMessage = async (content: string) => {
		await navigator.clipboard.writeText(content);
	};

	useEffect(() => {
		const element = scrollRef.current;
		if (!element) return;
		element.scrollTop = element.scrollHeight;
	}, [messages, streamingAssistantText, isTyping, conversation?.id]);

	if (!conversation) {
		return (
			<div className="w-full max-w-3xl">
				<div className="mb-6 text-left">
					<h1 className="text-ink text-[2.65rem] font-semibold tracking-tight">
						Let&apos;s get to work, James
					</h1>
					<p className="text-ink-dull mt-2 text-sm">
						Learn how to be productive with Spacebot. {''}
						<a
							href="https://github.com/spacedriveapp/spacebot"
							target="_blank"
							rel="noreferrer"
							className="text-ink-dull hover:text-ink underline underline-offset-4 transition-colors"
						>
							Read the docs.
						</a>
					</p>
				</div>

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
		);
	}

	return (
		<div className="relative flex h-full w-full max-w-4xl flex-col">
			<div className="pointer-events-none absolute inset-x-0 top-0 z-10 px-6 py-4">
				<div className="text-ink text-lg font-semibold">
					{conversation.title}
				</div>
				<div className="text-ink-dull mt-1 text-xs uppercase tracking-[0.14em]">
					{conversation.message_count} messages
				</div>
			</div>

			<div ref={scrollRef} className="flex-1 space-y-4 overflow-y-auto px-6">
				<div className="h-24 shrink-0" />
				{messages.length > 0 ? (
					messages.map((message) => {
						const isUser = message.role === 'user';
						return (
							<div
								key={message.id}
								className={`group flex flex-col ${isUser ? 'items-end' : 'items-start'}`}
							>
								<div
									className={`max-w-[80%] rounded-2xl px-4 py-3 text-[15px] leading-7 ${
										isUser
											? 'bg-accent text-white'
											: 'border-app-line bg-app text-ink border'
									}`}
								>
									<div className="whitespace-pre-wrap break-words">
										{message.content}
									</div>
								</div>
								{!isUser ? (
									<div className="mt-2 flex opacity-0 transition-opacity duration-150 group-hover:opacity-100">
										<TopBarButton
											icon={Copy}
											onClick={() => void copyMessage(message.content)}
											title="Copy message"
											className="h-7 w-7"
										/>
									</div>
								) : null}
							</div>
						);
					})
				) : (
					<div className="border-app-line bg-app text-ink-dull flex h-full min-h-[240px] items-center justify-center rounded-2xl border border-dashed text-sm">
						Start the conversation here.
					</div>
				)}

				{streamingAssistantText ? (
					<div className="flex justify-start">
						<div className="border-app-line bg-app text-ink max-w-[80%] rounded-2xl border px-4 py-3 text-[15px] leading-7">
							<div className="whitespace-pre-wrap break-words">
								{streamingAssistantText}
							</div>
						</div>
					</div>
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
					isSending={isSending}
				/>
			</div>
		</div>
	);
}
