import {CaretDown, Microphone, Sparkle} from '@phosphor-icons/react';
import {Popover} from '@sd/ui';
import {AnimatePresence, motion} from 'framer-motion';
import {useState} from 'react';

interface ChatComposerProps {
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
	showHeading?: boolean;
	isSending?: boolean;
}

export function ChatComposer({
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
	showHeading = true,
	isSending = false
}: ChatComposerProps) {
	const [isFocused, setIsFocused] = useState(false);
	const isExpanded = isFocused || draft.trim().length > 0;

	const canSend = !isSending && draft.trim().length > 0;

	return (
		<div className="border-app-line bg-app-box/70 rounded-[28px] border p-4 shadow-[0_30px_80px_rgba(0,0,0,0.22)] backdrop-blur-2xl">
			{showHeading && (
				<div className="text-ink-dull mb-3 flex items-center gap-2 px-1 text-xs font-medium">
					<Sparkle className="text-accent size-3.5" weight="fill" />
					What should Spacebot work on?
				</div>
			)}

			<div className="border-app-line bg-app rounded-[24px] border p-4">
				<motion.div
					animate={{height: isExpanded ? 140 : 90}}
					transition={{duration: 0.18, ease: 'easeOut'}}
					style={{overflow: 'hidden'}}
				>
					<textarea
						value={draft}
						onChange={(event) => onDraftChange(event.target.value)}
						onFocus={() => setIsFocused(true)}
						onBlur={() => setIsFocused(false)}
						onKeyDown={(event) => {
							if (event.key === 'Enter' && !event.shiftKey) {
								event.preventDefault();
								onSend();
							}
						}}
						placeholder="Ask Spacebot to review a project, plan work, or start a task..."
						className="text-ink placeholder:text-ink-faint h-full w-full resize-none border-0 bg-transparent text-base leading-7 outline-none focus:border-0 focus:outline-none focus:ring-0"
					/>
				</motion.div>

				<div className="mt-4 flex items-center justify-between gap-3">
					<div className="w-[210px]">
						<Popover
							popover={projectSelector}
							trigger={
								<button className="border-app-line bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink flex h-9 w-full items-center gap-2 rounded-full border px-3 text-left text-xs font-medium transition-colors">
									<span className="flex-1 truncate text-left">
										{selectedProject}
									</span>
									<CaretDown
										className="size-3"
										weight="bold"
									/>
								</button>
							}
							align="start"
							sideOffset={8}
							className="min-w-[220px] p-2"
						>
							<div className="space-y-1">
								{projectOptions.map((project) => (
									<button
										key={project}
										onClick={() => {
											onSelectProject(project);
											projectSelector.setOpen(false);
										}}
										className="text-ink hover:bg-app-selected w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm transition-colors"
									>
										{project}
									</button>
								))}
							</div>
						</Popover>
					</div>

					<motion.div layout className="flex items-center gap-2">
						<div className="w-[180px]">
							<Popover
								popover={modelSelector}
								trigger={
									<button className="border-app-line bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink flex h-9 w-full items-center gap-2 rounded-full border px-3 text-left text-xs font-medium transition-colors">
										<span className="flex-1 truncate text-left">
											{selectedModel}
										</span>
										<CaretDown
											className="size-3"
											weight="bold"
										/>
									</button>
								}
								align="end"
								sideOffset={8}
								className="min-w-[220px] p-2"
							>
								<div className="space-y-1">
									{modelOptions.map((model) => (
										<button
											key={model}
											onClick={() => {
												onSelectModel(model);
												modelSelector.setOpen(false);
											}}
											className="text-ink hover:bg-app-selected w-full cursor-pointer rounded-md px-3 py-2 text-left text-sm transition-colors"
										>
											{model}
										</button>
									))}
								</div>
							</Popover>
						</div>

						<button
							onClick={onOpenVoiceOverlay}
							className="border-app-line bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink flex size-9 items-center justify-center rounded-full border transition-colors"
						>
							<Microphone className="size-4" weight="fill" />
						</button>

						<AnimatePresence initial={false}>
							{canSend ? (
								<motion.div
									key="send-wrap"
									layout
									initial={{width: 0, opacity: 0, x: 12}}
									animate={{width: 76, opacity: 1, x: 0}}
									exit={{width: 0, opacity: 0, x: 12}}
									transition={{duration: 0.18, ease: 'easeOut'}}
									className="overflow-hidden"
								>
									<button
										onClick={onSend}
										className="border-app-line bg-accent hover:bg-accent-faint flex h-9 w-[76px] items-center justify-center rounded-full border px-4 text-xs font-medium text-white"
									>
										<span className="whitespace-nowrap">Send</span>
									</button>
								</motion.div>
							) : null}
						</AnimatePresence>
					</motion.div>
				</div>
			</div>
		</div>
	);
}
