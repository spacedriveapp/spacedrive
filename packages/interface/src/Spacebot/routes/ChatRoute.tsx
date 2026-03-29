import {ChatComposer} from '../ChatComposer';
import {EmptyChatHero} from '../EmptyChatHero';
import {useSpacebot} from '../SpacebotContext';

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
		models,
		composerProjectSelector,
		openVoiceOverlay
	} = useSpacebot();

	return (
		<div className="flex h-full w-full items-center justify-center py-10">
			<div className="w-full max-w-3xl px-6">
				<EmptyChatHero />

				<ChatComposer
					draft={draft}
					onDraftChange={setDraft}
					onSend={() => void handleSendMessage()}
					onOpenVoiceOverlay={openVoiceOverlay}
					selectedProject={selectedProject}
					selectedModel={selectedModel}
					projectOptions={projectOptions}
					models={models}
					onSelectProject={setSelectedProject}
					onSelectModel={setSelectedModel}
					projectSelector={composerProjectSelector}
					showHeading={false}
					isSending={isSending}
				/>
			</div>
		</div>
	);
}
