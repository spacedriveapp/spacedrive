import {CaretDown, Microphone, Sparkle} from '@phosphor-icons/react';
import {Popover} from '@sd/ui';

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
	return (
		<div className="border-app-line bg-app-box/90 rounded-[28px] border p-4 shadow-[0_30px_80px_rgba(0,0,0,0.25)] backdrop-blur-xl">
			{showHeading && (
				<div className="text-ink-dull mb-3 flex items-center gap-2 px-1 text-xs font-medium">
					<Sparkle className="text-accent size-3.5" weight="fill" />
					What should Spacebot work on?
				</div>
			)}

			<div className="border-app-line bg-app rounded-[24px] border p-4">
				<textarea
					value={draft}
					onChange={(event) => onDraftChange(event.target.value)}
					onKeyDown={(event) => {
						if (event.key === 'Enter' && !event.shiftKey) {
							event.preventDefault();
							onSend();
						}
					}}
					placeholder="Ask Spacebot to review a project, plan work, or start a task..."
					className="text-ink placeholder:text-ink-faint min-h-[140px] w-full resize-none border-0 bg-transparent text-base leading-7 outline-none focus:border-0 focus:outline-none focus:ring-0"
				/>

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

					<div className="flex items-center gap-2">
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

						<button
							onClick={onSend}
							disabled={isSending || draft.trim().length === 0}
							className="border-app-line bg-accent hover:bg-accent-faint rounded-full border px-3 py-1.5 text-xs font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
						>
							Send
						</button>
					</div>
				</div>
			</div>
		</div>
	);
}
