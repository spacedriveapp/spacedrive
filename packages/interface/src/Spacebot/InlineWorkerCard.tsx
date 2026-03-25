import {CaretDown, CheckCircle, ClockCounterClockwise, Copy, Stop, Wrench} from '@phosphor-icons/react';
import {apiClient, type WorkerListItem} from '@spacebot/api-client';
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query';
import {AnimatePresence, motion} from 'framer-motion';
import clsx from 'clsx';
import {useMemo, useState} from 'react';
import {TopBarButton, TopBarButtonGroup} from '@sd/ui';

import {ToolCall, pairTranscriptSteps} from './ToolCall';

export function InlineWorkerCard({agentId, worker}: {agentId: string; worker: WorkerListItem}) {
	const [expanded, setExpanded] = useState(false);
	const queryClient = useQueryClient();
	const detailQuery = useQuery({
		queryKey: ['spacebot', 'worker-detail', agentId, worker.id],
		queryFn: () => apiClient.workerDetail(agentId, worker.id),
		enabled: expanded,
		refetchInterval: worker.status === 'running' ? 1500 : false,
	});

	const toolCalls = useMemo(() => {
		const transcript = detailQuery.data?.transcript ?? [];
		return pairTranscriptSteps(transcript);
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

	const isRunning = worker.status === 'running';
	const isDone = worker.status === 'completed';

	return (
		<div className="group flex flex-col items-start">
			<div className="border-app-line/50 bg-app-box/30 overflow-hidden rounded-2xl border backdrop-blur-sm">
				<button
					onClick={() => setExpanded((value) => !value)}
					className="flex w-full items-start gap-3 px-4 py-3 text-left transition-colors hover:bg-app-box/30"
				>
					<div className="mt-0.5 shrink-0">
						{isRunning ? (
							<div className="bg-accent/15 text-accent flex size-7 items-center justify-center rounded-full">
								<ClockCounterClockwise className="size-4 animate-spin" weight="bold" />
							</div>
						) : isDone ? (
							<div className="bg-emerald-500/15 text-emerald-400 flex size-7 items-center justify-center rounded-full">
								<CheckCircle className="size-4" weight="fill" />
							</div>
						) : (
							<div className="bg-app-hover text-ink-dull flex size-7 items-center justify-center rounded-full">
								<Wrench className="size-4" weight="bold" />
							</div>
						)}
					</div>

					<div className="min-w-0 flex-1">
						<div className="flex items-center gap-2">
							<div className="text-ink line-clamp-2 min-w-0 flex-1 text-sm font-medium leading-5">
								{worker.task}
							</div>
							<span
								className={clsx(
									'rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.12em]',
									isRunning
										? 'bg-accent/12 text-accent'
										: isDone
											? 'bg-emerald-500/12 text-emerald-400'
											: 'bg-app-hover text-ink-dull'
								)}
							>
								{worker.status}
							</span>
						</div>
						<div className="text-ink-dull mt-1 flex items-center gap-2 text-xs">
							<span>{worker.tool_calls} tool calls</span>
							{worker.live_status ? <span className="text-ink-faint truncate">{worker.live_status}</span> : null}
						</div>
					</div>

					<CaretDown className={clsx('text-ink-faint mt-1 size-4 shrink-0 transition-transform', expanded ? 'rotate-180' : '')} weight="bold" />
				</button>

				<AnimatePresence initial={false}>
					{expanded ? (
						<motion.div
							initial={{height: 0, opacity: 0}}
							animate={{height: 'auto', opacity: 1}}
							exit={{height: 0, opacity: 0}}
							transition={{duration: 0.18, ease: 'easeOut'}}
							className="overflow-hidden"
						>
							<div className="border-app-line/30 flex flex-col gap-2 border-t px-4 py-3">
								{detailQuery.isLoading ? (
									<div className="text-ink-faint text-xs">Loading worker transcript...</div>
								) : toolCalls.length > 0 ? (
									toolCalls.map((pair) => <ToolCall key={pair.id} pair={pair} />)
								) : (
									<div className="text-ink-faint text-xs">No tool calls yet.</div>
								)}
							</div>
						</motion.div>
					) : null}
				</AnimatePresence>
			</div>

			<div className="mt-2 flex opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100">
				<TopBarButtonGroup>
					<TopBarButton
						icon={Copy}
						onClick={() => void copyLogs()}
						title="Copy logs"
						disabled={!detailQuery.data}
					/>
					{isRunning ? (
						<TopBarButton
							icon={Stop}
							onClick={() => void cancelMutation.mutateAsync()}
							title="Cancel worker"
							disabled={!worker.channel_id || cancelMutation.isPending}
						/>
					) : null}
				</TopBarButtonGroup>
			</div>
		</div>
	);
}
