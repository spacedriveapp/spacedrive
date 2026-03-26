import {apiClient, type WorkerListItem} from '@spacebot/api-client';
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query';
import {useMemo, useState} from 'react';

import {
	InlineWorkerCard as SpaceUIInlineWorkerCard,
	type TaskInfo,
	type TranscriptStep,
} from '@spaceui/ai';

export function InlineWorkerCard({agentId, worker}: {agentId: string; worker: WorkerListItem}) {
	const [expanded, setExpanded] = useState(false);
	const queryClient = useQueryClient();
	const detailQuery = useQuery({
		queryKey: ['spacebot', 'worker-detail', agentId, worker.id],
		queryFn: () => apiClient.workerDetail(agentId, worker.id),
		enabled: expanded,
		refetchInterval: worker.status === 'running' ? 1500 : false,
	});

	const task = useMemo<TaskInfo>(
		() => ({
			id: worker.id,
			title: worker.task,
			status: worker.status,
			priority: 'medium',
			assignees: [],
			conversation_id: worker.channel_id ?? undefined,
		}),
		[worker.channel_id, worker.id, worker.status, worker.task]
	);

	const transcript = useMemo<TranscriptStep[]>(() => {
		return (detailQuery.data?.transcript ?? []).map((step) => ({
			type: step.type,
			call_id: step.call_id,
			name: step.name,
			content: step.content,
			text: step.text,
		}));
	}, [detailQuery.data?.transcript]);

	const copyLogs = async () => {
		const detail = detailQuery.data;
		if (!detail) return;

		const transcriptText = (detail.transcript ?? [])
			.map((step) => JSON.stringify(step, null, 2))
			.join('\n\n');

		const payload = [
			`Worker: ${detail.task}`,
			`Status: ${detail.status}`,
			`Started: ${detail.started_at}`,
			detail.completed_at ? `Completed: ${detail.completed_at}` : null,
			detail.result ? `Result:\n${detail.result}` : null,
			transcriptText ? `Transcript:\n${transcriptText}` : null,
		]
			.filter(Boolean)
			.join('\n\n');

		await navigator.clipboard.writeText(payload);
	};

	const cancelMutation = useMutation({
		mutationFn: () =>
			apiClient.cancelProcess({
				channelId: worker.channel_id ?? '',
				processType: 'worker',
				processId: worker.id,
			}),
		onSuccess: async () => {
			await Promise.all([
				queryClient.invalidateQueries({queryKey: ['spacebot', 'conversation-workers', agentId, worker.channel_id]}),
				queryClient.invalidateQueries({queryKey: ['spacebot', 'worker-detail', agentId, worker.id]}),
				queryClient.invalidateQueries({queryKey: ['spacebot', 'channel-timeline', worker.channel_id]}),
			]);
		},
	});

	return (
		<SpaceUIInlineWorkerCard
			task={task}
			transcript={transcript}
			expanded={expanded}
			onExpandedChange={setExpanded}
			onCopyLogs={() => void copyLogs()}
			onCancel={worker.status === 'running' && worker.channel_id ? () => void cancelMutation.mutateAsync() : undefined}
		/>
	);
}
