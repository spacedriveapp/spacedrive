/**
 * Stand-in for `@spacebot/api-client`, which lives in the separate
 * spacedriveapp/spacebot repository and is not published to a registry.
 *
 * The type surface mirrors what this tree consumes. Every call rejects, so the
 * Spacebot routes render their error states instead of talking to a backend
 * that is not there. To develop against a real instance, point the
 * `@spacebot/api-client` alias in the tsconfig and vite resolve blocks at a
 * local checkout of that repository.
 */

export type ProcessType = 'channel' | 'branch' | 'worker';

export interface InboundMessageEvent {
	type: 'inbound_message';
	agent_id: string;
	channel_id: string;
	sender_id: string;
	sender_name?: string | null;
	text: string;
}

export interface OutboundMessageEvent {
	type: 'outbound_message';
	agent_id: string;
	channel_id: string;
	text: string;
}

export interface OutboundMessageDeltaEvent {
	type: 'outbound_message_delta';
	agent_id: string;
	channel_id: string;
	text_delta: string;
	aggregated_text: string;
}

export interface TypingStateEvent {
	type: 'typing_state';
	agent_id: string;
	channel_id: string;
	is_typing: boolean;
}

export type TimelineItem =
	| {
			type: 'message';
			id: string;
			content: string;
			created_at: string;
			role: string;
			sender_id?: string | null;
			sender_name?: string | null;
	  }
	| {
			type: 'worker_run';
			id: string;
			task: string;
			status: string;
			started_at: string;
			completed_at?: string | null;
			result?: string | null;
	  }
	| {
			type: 'branch_run';
			id: string;
			description: string;
			started_at: string;
			completed_at?: string | null;
			conclusion?: string | null;
	  }
	| {
			type: 'tool_call_run';
			id: string;
			tool_name: string;
			args: string;
			status: string;
			started_at: string;
			completed_at?: string | null;
			result?: string | null;
	  };

export interface MessagesResponse {
	items: TimelineItem[];
}

export interface WorkerListItem {
	id: string;
	task: string;
	status: string;
	worker_type: string;
	tool_calls: number;
	started_at: string;
	completed_at?: string | null;
	channel_id?: string | null;
	channel_name?: string | null;
	has_transcript?: boolean;
	live_status?: string | null;
	opencode_port?: number | null;
	opencode_session_id?: string | null;
	directory?: string | null;
	interactive?: boolean;
	project_id?: string | null;
	project_name?: string | null;
}

export interface WorkerListResponse {
	workers: WorkerListItem[];
}

export interface WorkerDetailResponse {
	id: string;
	task: string;
	status: string;
	started_at: string;
	completed_at?: string | null;
	result?: string | null;
	transcript?: unknown[] | null;
}

export interface PortalConversationSummary {
	id: string;
	agent_id: string;
	title: string;
	title_source: string;
	archived: boolean;
	message_count: number;
	created_at: string;
	updated_at: string;
	last_message_at?: string | null;
	last_message_preview?: string | null;
	last_message_role?: string | null;
}

export interface PortalConversationsResponse {
	conversations: PortalConversationSummary[];
}

export interface PortalConversationResponse {
	conversation: PortalConversationSummary;
}

export interface PortalHistoryMessage {
	id: string;
	role: string;
	content: string;
	created_at: string;
	sender_name?: string | null;
}

export interface Subtask {
	title: string;
	completed: boolean;
}

export interface Task {
	id: string;
	task_number: number;
	title: string;
	description?: string | null;
	status: string;
	priority: string;
	owner_agent_id: string;
	assigned_agent_id: string;
	subtasks: Subtask[];
	metadata: unknown;
	worker_id?: string | null;
	created_by: string;
	created_at: string;
	updated_at: string;
	completed_at?: string | null;
}

export interface TaskListResponse {
	tasks: Task[];
}

export interface UpdateTaskRequest {
	title?: string;
	description?: string | null;
	status?: string;
	priority?: string;
	assigned_agent_id?: string;
	complete_subtask?: number;
}

export interface TtsProfile {
	id: string;
	name?: string | null;
	default_engine?: string | null;
	preset_engine?: string | null;
	effects_chain?: unknown;
}

let baseUrl = '';

export function setServerUrl(url: string) {
	baseUrl = url.replace(/\/+$/, '');
}

export function getEventsUrl() {
	return `${baseUrl}/api/events`;
}

function unavailable<T>(): Promise<T> {
	return Promise.reject(new Error('Spacebot is not linked to this build'));
}

export const apiClient = {
	channelMessages(_channelId: string, _limit = 200, _before?: string) {
		return unavailable<MessagesResponse>();
	},

	portalHistory(_agentId: string, _sessionId: string, _limit = 100) {
		return unavailable<PortalHistoryMessage[]>();
	},

	portalSend(_input: {
		agentId: string;
		sessionId: string;
		senderName?: string;
		message: string;
	}) {
		return unavailable<{ok: boolean}>();
	},

	listPortalConversations(
		_agentId: string,
		_includeArchived = false,
		_limit = 100
	) {
		return unavailable<PortalConversationsResponse>();
	},

	createPortalConversation(_input: {agentId: string; title?: string | null}) {
		return unavailable<PortalConversationResponse>();
	},

	listTasks(_agentId: string, _limit = 20) {
		return unavailable<TaskListResponse>();
	},

	listWorkers(_input: {
		agentId: string;
		limit?: number;
		offset?: number;
		status?: string;
	}) {
		return unavailable<WorkerListResponse>();
	},

	workerDetail(_agentId: string, _workerId: string) {
		return unavailable<WorkerDetailResponse>();
	},

	cancelProcess(_input: {
		channelId: string;
		processType: ProcessType;
		processId: string;
	}) {
		return unavailable<{success: boolean; message: string}>();
	},

	webChatSendAudio(
		_agentId: string,
		_sessionId: string,
		_audioBlob: Blob,
		_senderName?: string
	) {
		return unavailable<Response>();
	},

	ttsGenerate(
		_text: string,
		_options?: {profileId?: string; engine?: string; agentId?: string}
	) {
		return unavailable<ArrayBuffer>();
	},

	ttsProfiles(_agentId?: string) {
		return unavailable<TtsProfile[]>();
	}
};
