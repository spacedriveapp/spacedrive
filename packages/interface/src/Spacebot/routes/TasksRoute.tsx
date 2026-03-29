import {
	apiClient,
	type Task,
	type TaskStatus,
	type UpdateTaskRequest,
} from '@spacebot/api-client';
import {
	TaskDetail,
	TaskList,
	type TaskPriority as UiTaskPriority,
	type TaskStatus as UiTaskStatus,
} from '@spaceui/ai';
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query';
import {useCallback, useState} from 'react';
import {agents, useSpacebot} from '../SpacebotContext';

function resolveAgentName(agentId: string): string {
	return agents.find((a) => a.id === agentId)?.name ?? agentId;
}

export function TasksRoute() {
	const {selectedAgent} = useSpacebot();
	const queryClient = useQueryClient();
	const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
	const [collapsedGroups, setCollapsedGroups] = useState<Set<UiTaskStatus>>(
		() => new Set()
	);

	const queryKey = ['spacebot', 'tasks', selectedAgent];

	const tasksQuery = useQuery({
		queryKey,
		queryFn: () => apiClient.listTasks({agent_id: selectedAgent, limit: 200}),
		refetchInterval: 5000,
	});

	const tasks = (tasksQuery.data?.tasks ?? []) as unknown as Array<
		Task & {status: UiTaskStatus; priority: UiTaskPriority}
	>;
	const activeTask = tasks.find((t) => t.id === activeTaskId);

	const invalidate = useCallback(
		() => queryClient.invalidateQueries({queryKey}),
		[queryClient, queryKey]
	);

	const updateMutation = useMutation({
		mutationFn: ({taskNumber, req}: {taskNumber: number; req: UpdateTaskRequest}) =>
			apiClient.updateTask(taskNumber, req),
		onSuccess: () => void invalidate(),
	});

	const deleteMutation = useMutation({
		mutationFn: (taskNumber: number) => apiClient.deleteTask(taskNumber),
		onSuccess: () => {
			setActiveTaskId(null);
			void invalidate();
		},
	});

	const handleStatusChange = useCallback(
		(task: {task_number: number}, status: UiTaskStatus) => {
			updateMutation.mutate({
				taskNumber: task.task_number,
				req: {status},
			});
		},
		[updateMutation]
	);

	const handleDelete = useCallback(
		(task: {task_number: number}) => {
			deleteMutation.mutate(task.task_number);
		},
		[deleteMutation]
	);

	const handleSubtaskToggle = useCallback(
		(task: {task_number: number}, index: number, _completed: boolean) => {
			updateMutation.mutate({
				taskNumber: task.task_number,
				req: {complete_subtask: index},
			});
		},
		[updateMutation]
	);

	const handleToggleGroup = useCallback((status: UiTaskStatus) => {
		setCollapsedGroups((prev) => {
			const next = new Set(prev);
			if (next.has(status)) next.delete(status);
			else next.add(status);
			return next;
		});
	}, []);

	return (
		<div className="flex h-full w-full gap-0">
			{/* List panel */}
			<div className="flex min-w-0 flex-1 flex-col">
				{tasksQuery.isLoading ? (
					<div className="py-8 text-center text-sm text-ink-faint">
						Loading tasks...
					</div>
				) : tasksQuery.error ? (
					<div className="py-8 text-center text-sm text-red-400">
						Failed to load tasks.
						<div className="mt-1 font-mono text-[10px] text-ink-faint">
							{(tasksQuery.error as Error).message}
						</div>
					</div>
				) : tasks.length === 0 ? (
					<div className="flex flex-1 items-center justify-center">
						<div className="text-center">
							<p className="text-ink-dull text-sm">No tasks yet.</p>
							<p className="text-ink-faint mt-1 text-xs">
								Create one to get started.
							</p>
						</div>
					</div>
				) : (
					<div className="flex-1 overflow-y-auto">
						<TaskList
							tasks={tasks}
							activeTaskId={activeTaskId ?? undefined}
							collapsedGroups={collapsedGroups}
							onToggleGroup={handleToggleGroup}
							onTaskClick={(task) => setActiveTaskId(task.id)}
							onStatusChange={handleStatusChange}
							onDelete={handleDelete}
							resolveAgentName={resolveAgentName}
						/>
					</div>
				)}
			</div>

			{/* Detail panel */}
			{activeTask && (
				<div className="border-app-line w-[400px] shrink-0 overflow-y-auto border-l">
					<TaskDetail
						task={activeTask}
						resolveAgentName={resolveAgentName}
						onStatusChange={handleStatusChange}
						onSubtaskToggle={handleSubtaskToggle}
						onDelete={handleDelete}
						onClose={() => setActiveTaskId(null)}
					/>
				</div>
			)}
		</div>
	);
}
