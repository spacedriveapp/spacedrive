import { useParams } from 'react-router-dom';
import { ConversationScreen } from '../ConversationScreen';
import { useSpacebot } from '../SpacebotContext';

export function ConversationRoute() {
	const params = useParams();
	// With splat route "conversation/*", the conversation ID is in params["*"]
	const conversationId = decodeURIComponent(params["*"] || "");
	const {
		selectedAgent,
		getConversationById,
		getConversationMessages,
		isTyping,
		streamingAssistantText,
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

	const conversation = conversationId ? getConversationById(conversationId) : null;
	const messages = conversationId ? getConversationMessages(conversationId) : [];

	return (
		<ConversationScreen
			agentId={selectedAgent}
			conversation={conversation}
			messages={messages}
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
			isSending={isSending}
		/>
	);
}
