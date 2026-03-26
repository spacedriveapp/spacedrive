import { ChatComposer } from '../ChatComposer';
import { EmptyChatHero } from '../EmptyChatHero';
import { useSpacebot } from '../SpacebotContext';
import { ChatComposer as SpaceUIChatComposer, type ModelOption } from '@spaceui/ai';
import { Popover } from '@sd/ui';
import { OptionList, OptionListItem, SelectTriggerButton } from '@spaceui/primitives';

export function ChatRoute() {
	const { 
		draft, 
		setDraft, 
		handleSendMessage, 
		isSending,
		selectedProject,
		setSelectedProject,
		selectedModel,
		setSelectedModel,
		projectOptions,
		modelOptions,
		composerProjectSelector,
		composerModelSelector,
		openVoiceOverlay
	} = useSpacebot();

	const comparisonModels: ModelOption[] = modelOptions.map((model) => ({
		id: model,
		name: model,
		provider: 'spacebot',
	}));

	return (
		<div className="flex h-full w-full items-center justify-center py-10">
			<div className="w-full max-w-[1480px] px-6">
				<EmptyChatHero />

				<div className="grid gap-8 xl:grid-cols-2">
					<div>
						<div className="text-ink mb-3 text-sm font-medium">Original</div>
						<ChatComposer
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
							isSending={isSending}
						/>
					</div>

					<div>
						<div className="text-ink mb-3 text-sm font-medium">SpaceUI</div>
						<SpaceUIChatComposer
							value={draft}
							onChange={setDraft}
							onSend={() => void handleSendMessage()}
							footerStart={
								<div className="w-[210px] max-w-full">
									<Popover
										popover={composerProjectSelector}
										trigger={
											<SelectTriggerButton className="w-full min-w-0">{selectedProject}</SelectTriggerButton>
										}
										align="start"
										sideOffset={8}
										className="min-w-[220px] p-2"
									>
										<OptionList>
											{projectOptions.map((project) => (
												<OptionListItem
													key={project}
													onClick={() => {
														setSelectedProject(project);
														composerProjectSelector.setOpen(false);
													}}
													selected={project === selectedProject}
												>
													{project}
												</OptionListItem>
											))}
										</OptionList>
									</Popover>
								</div>
							}
							onVoiceClick={openVoiceOverlay}
							disabled={isSending}
							placeholder="Ask Spacebot to review a project, plan work, or start a task..."
							models={comparisonModels}
							selectedModel={selectedModel}
							onModelChange={setSelectedModel}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}
