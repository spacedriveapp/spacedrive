import {Microphone, Sparkle} from '@phosphor-icons/react';
import {ModelSelector, type ModelOption} from '@spaceui/ai';
import {
	CircleButton,
	OptionList,
	OptionListItem,
	Popover,
	SelectPill,
	usePopover
} from '@spaceui/primitives';
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
	models: ModelOption[];
	onSelectProject(project: string): void;
	onSelectModel(model: string): void;
	projectSelector: ReturnType<typeof usePopover>;
	showHeading?: boolean;
	showOuterBox?: boolean;
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
	models,
	onSelectProject,
	onSelectModel,
	projectSelector,
	showHeading = true,
	showOuterBox = true,
	isSending = false
}: ChatComposerProps) {
	const [isFocused, setIsFocused] = useState(false);
	const isExpanded = isFocused || draft.trim().length > 0;

	const canSend = !isSending && draft.trim().length > 0;
	const composerBody = (
		<>
			{showHeading && (
				<div className="text-ink-dull mb-3 flex items-center gap-2 px-1 text-xs font-medium">
					<Sparkle className="text-accent size-3.5" weight="fill" />
					What should Spacebot work on?
				</div>
			)}

			<div
				className={`border-app-line rounded-[24px] border p-4 ${
					showOuterBox
						? 'bg-app'
						: 'bg-app-box/70 shadow-[0_20px_60px_rgba(0,0,0,0.18)] backdrop-blur-2xl'
				}`}
			>
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
						<Popover.Root
							open={projectSelector.open}
							onOpenChange={projectSelector.setOpen}
						>
							<Popover.Trigger asChild>
								<SelectPill className="w-full">
									{selectedProject}
								</SelectPill>
							</Popover.Trigger>
							<Popover.Content align="start" sideOffset={8}>
								<OptionList>
									{projectOptions.map((project) => (
										<OptionListItem
											key={project}
											selected={
												project === selectedProject
											}
											onClick={() => {
												onSelectProject(project);
												projectSelector.setOpen(false);
											}}
										>
											{project}
										</OptionListItem>
									))}
								</OptionList>
							</Popover.Content>
						</Popover.Root>
					</div>

					<motion.div layout className="flex items-center gap-2">
						<div className="w-[180px]">
							<ModelSelector
								models={models}
								value={selectedModel}
								onChange={onSelectModel}
							/>
						</div>

						<CircleButton
							icon={Microphone}
							onClick={onOpenVoiceOverlay}
						/>

						<AnimatePresence initial={false}>
							{canSend ? (
								<motion.div
									key="send-wrap"
									layout
									initial={{width: 0, opacity: 0, x: 12}}
									animate={{width: 76, opacity: 1, x: 0}}
									exit={{width: 0, opacity: 0, x: 12}}
									transition={{
										duration: 0.18,
										ease: 'easeOut'
									}}
									className="overflow-hidden"
								>
									<button
										onClick={onSend}
										className="border-app-line bg-accent hover:bg-accent-faint flex h-9 w-[76px] items-center justify-center rounded-full border px-4 text-xs font-medium text-white"
									>
										<span className="whitespace-nowrap">
											Send
										</span>
									</button>
								</motion.div>
							) : null}
						</AnimatePresence>
					</motion.div>
				</div>
			</div>
		</>
	);

	if (!showOuterBox) return composerBody;

	return (
		<div className="border-app-line bg-app-box/70 rounded-[28px] border p-4 shadow-[0_30px_80px_rgba(0,0,0,0.22)] backdrop-blur-2xl">
			{composerBody}
		</div>
	);
}
