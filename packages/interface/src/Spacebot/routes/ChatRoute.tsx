import { ChatComposer } from '../ChatComposer';
import { useSpacebot } from '../SpacebotContext';

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

	return (
		<div className="w-full max-w-3xl">
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
	);
}
