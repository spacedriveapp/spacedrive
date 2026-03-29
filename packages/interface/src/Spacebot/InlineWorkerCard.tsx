import {apiClient, type WorkerListItem} from '@spacebot/api-client';
import {
	InlineWorkerCard as InlineWorkerCardUI,
	type TranscriptStep
} from '@spaceui/ai';
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query';

export function InlineWorkerCard({
	agentId,
	worker
}: {
	agentId: string;
	worker: WorkerListItem;
}) {
	const queryClient = useQueryClient();
	const detailQuery = useQuery({
		queryKey: ['spacebot', 'worker-detail', agentId, worker.id],
		queryFn: () => apiClient.workerDetail(agentId, worker.id),
		refetchInterval: worker.status === 'running' ? 1500 : false
	});

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
			transcriptText ? `Transcript:\n${transcriptText}` : null
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
				processId: worker.id
			}),
		onSuccess: async () => {
			await Promise.all([
				queryClient.invalidateQueries({
					queryKey: [
						'spacebot',
						'conversation-workers',
						agentId,
						worker.channel_id
					]
				}),
				queryClient.invalidateQueries({
					queryKey: ['spacebot', 'worker-detail', agentId, worker.id]
				}),
				queryClient.invalidateQueries({
					queryKey: ['spacebot', 'channel-timeline', worker.channel_id]
				})
			]);
		}
	});

	const isRunning = worker.status === 'running';
	const canCancel = isRunning && !!worker.channel_id && !cancelMutation.isPending;

	return (
		<InlineWorkerCardUI
			title={worker.task}
			status={worker.status}
			toolCallCount={worker.tool_calls}
			liveStatus={worker.live_status}
			transcript={(detailQuery.data?.transcript ?? []) as TranscriptStep[]}
			isTranscriptLoading={detailQuery.isLoading}
			onCopyLogs={detailQuery.data ? () => void copyLogs() : undefined}
			onCancel={canCancel ? () => void cancelMutation.mutateAsync() : undefined}
		/>
	);
}
