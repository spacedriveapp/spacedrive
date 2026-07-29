/**
 * Stub for @spacebot/api-client when the spacebot repo is not checked out locally.
 * Spacebot UI features are disabled; core Spacedrive remains usable.
 */

export type InboundMessageEvent = Record<string, unknown>;
export type OutboundMessageDeltaEvent = Record<string, unknown>;
export type OutboundMessageEvent = Record<string, unknown>;
export type PortalConversationResponse = {
	conversation: { id: string; title?: string | null };
};
export type PortalConversationSummary = {
	id: string;
	title?: string | null;
	updated_at?: string;
};
export type PortalHistoryMessage = Record<string, unknown>;
export type TypingStateEvent = Record<string, unknown>;
export type TimelineItem = {
	type: string;
	id: string;
	[key: string]: unknown;
};
export type WorkerListItem = {
	id: string;
	channel_id?: string | null;
	status: string;
	worker_type?: string;
	task?: string;
	[key: string]: unknown;
};
export type Task = Record<string, unknown>;
export type UpdateTaskRequest = Record<string, unknown>;
export type TtsProfile = { id: string; name?: string; [key: string]: unknown };

let serverUrl = 'http://127.0.0.1:19898';

export function setServerUrl(url: string): void {
	serverUrl = url;
}

export function getEventsUrl(): string {
	return `${serverUrl}/api/events`;
}

const emptyBuffer = new ArrayBuffer(0);

export const apiClient = {
	listPortalConversations: async (_agentId: string, _includeArchived: boolean, _limit: number) => ({
		conversations: [] as PortalConversationSummary[],
	}),
	portalHistory: async (_agentId: string, _conversationId: string, _limit: number) => [],
	createPortalConversation: async (_req: {
		agentId: string;
		title?: string | null;
	}): Promise<PortalConversationResponse> => ({
		conversation: { id: 'stub-conversation', title: null },
	}),
	portalSend: async (_req: Record<string, unknown>) => undefined,
	channelMessages: async (_channelId: string, _limit: number) => ({
		items: [] as TimelineItem[],
	}),
	listWorkers: async (_req: { agentId: string; limit?: number }) => ({
		workers: [] as WorkerListItem[],
	}),
	workerDetail: async (_agentId: string, _workerId: string) => ({
		id: 'stub-worker',
		task: 'Spacebot not available',
		status: 'completed',
		started_at: new Date().toISOString(),
		transcript: [],
	}),
	cancelProcess: async (_req: Record<string, unknown>) => undefined,
	listTasks: async (_agentId: string, _limit: number) => ({
		tasks: [] as Task[],
	}),
	updateTask: async (_taskNumber: number, _req: UpdateTaskRequest) => undefined,
	deleteTask: async (_taskNumber: number) => undefined,
	ttsProfiles: async (_agentId: string) => [] as TtsProfile[],
	ttsGenerate: async (_text: string, _opts?: Record<string, unknown>) => emptyBuffer,
	webChatSendAudio: async (
		_agentId: string,
		_sessionId: string,
		_blob: Blob,
	) => ({ ok: false, status: 503 }),
};

export default apiClient;