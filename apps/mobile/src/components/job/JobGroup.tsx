import { Folder } from '@sd/assets/icons';
import dayjs from 'dayjs';
import { DotsThreeVertical, Eye, Pause, Play, Stop, Trash } from 'phosphor-react-native';
import { SetStateAction, useMemo, useState } from 'react';
import { Animated, Pressable, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import {
	getJobNiceActionName,
	getTotalTasks,
	JobGroup,
	JobProgressEvent,
	Report,
	useLibraryMutation,
	useRspcLibraryContext,
	useTotalElapsedTimeText
} from '@sd/client';
import { tw, twStyle } from '~/lib/tailwind';

import { AnimatedHeight } from '../animation/layout';
import { ProgressBar } from '../animation/ProgressBar';
import { Button } from '../primitive/Button';
import { Menu, MenuItem } from '../primitive/Menu';
import { toast } from '../primitive/Toast';
import Job from './Job';
import JobContainer from './JobContainer';

interface JobGroupProps {
	group: JobGroup;
	progress: Record<string, JobProgressEvent>;
}

export default function ({ group, progress }: JobGroupProps) {
	const { jobs } = group;

	const [showChildJobs, setShowChildJobs] = useState(false);

	const runningJob = jobs.find((job) => job.status === 'Running');

	const tasks = getTotalTasks(jobs);
	const totalGroupTime = useTotalElapsedTimeText(jobs);

	const dateStarted = useMemo(() => {
		const createdAt = dayjs(jobs[0]?.created_at).fromNow();
		return createdAt.charAt(0).toUpperCase() + createdAt.slice(1);
	}, [jobs]);

	if (jobs.length === 0) return <></>;

	const renderRightActions = (
		progress: Animated.AnimatedInterpolation<number>,
		_dragX: Animated.AnimatedInterpolation<number>,
		swipeable: Swipeable
	) => {
		const translate = progress.interpolate({
			inputRange: [0, 1],
			outputRange: [100, 0],
			extrapolate: 'clamp'
		});

		return (
			<Animated.View
				style={[
					tw`mt-5 flex flex-row items-start pr-4`,
					{ transform: [{ translateX: translate }] }
				]}
			>
				<Options
					showChildJobs={showChildJobs}
					setShowChildJobs={setShowChildJobs}
					activeJob={runningJob}
					group={group}
				/>
			</Animated.View>
		);
	};

	return (
		<Swipeable
			containerStyle={tw.style(showChildJobs && 'bg-app-darkBox/30')}
			enableTrackpadTwoFingerGesture
			renderRightActions={renderRightActions}
		>
			{jobs?.length > 1 ? (
				<>
					<Pressable onPress={() => setShowChildJobs((v) => !v)}>
						<JobContainer
							icon={Folder}
							containerStyle={tw.style('pb-2', showChildJobs && 'border-b-0 pb-1')}
							name={getJobNiceActionName(
								group.action ?? '',
								group.status === 'Completed',
								jobs[0]
							)}
							textItems={[
								[
									{
										text: `${tasks.total} ${
											tasks.total <= 1 ? 'task' : 'tasks'
										}`
									},
									{ text: dateStarted },
									{ text: totalGroupTime || undefined },

									{
										text: ['Queued', 'Paused', 'Canceled', 'Failed'].includes(
											group.status
										)
											? group.status
											: undefined
									}
								],
								[
									{
										text:
											!showChildJobs && runningJob !== undefined
												? progress[runningJob.id]?.message
												: undefined
									}
								]
							]}
						>
							{!showChildJobs && runningJob && (
								<View style={tw`mb-2 ml-1.5`}>
									<ProgressBar
										pending={tasks.completed === 0}
										value={tasks.completed}
										total={tasks.total}
									/>
								</View>
							)}
						</JobContainer>
					</Pressable>
					{showChildJobs && (
						<AnimatedHeight>
							{jobs.map((job, i) => (
								<View style={tw`relative`} key={job.id}>
									<View
										style={twStyle(
											`absolute bottom-0 left-9 top-0.5 w-px bg-app-button`,
											{
												height: i === jobs.length - 1 ? 28 : '100%'
											}
										)}
									/>
									<View
										style={tw`top-7.5 absolute left-9 h-px w-4 bg-app-button`}
									/>
									<Job
										containerStyle={tw`ml-5`}
										isChild={jobs.length > 1}
										job={job}
										progress={progress[job.id] ?? null}
									/>
								</View>
							))}
						</AnimatedHeight>
					)}
				</>
			) : (
				<Job job={jobs[0]!} progress={progress[jobs[0]!.id] || null} />
			)}
		</Swipeable>
	);
}

const toastErrorSuccess = (
	errorMessage?: string,
	successMessage?: string,
	successCallBack?: () => void
) => {
	return {
		onError: () => {
			errorMessage && toast.error(errorMessage);
		},
		onSuccess: () => {
			successMessage && toast.success(successMessage), successCallBack?.();
		}
	};
};

interface OptionsProps {
	activeJob?: Report;
	group: JobGroup;
	showChildJobs: boolean;
	setShowChildJobs: React.Dispatch<SetStateAction<boolean>>;
}

function Options({ activeJob, group, setShowChildJobs, showChildJobs }: OptionsProps) {
	const rspc = useRspcLibraryContext();

	const clearJob = useLibraryMutation(['jobs.clear'], {
		onSuccess: () => {
			rspc.queryClient.invalidateQueries(['jobs.reports']);
		}
	});

	const resumeJob = useLibraryMutation(
		['jobs.resume'],
		toastErrorSuccess('failed to resume job', 'job has been resumed')
	);
	const pauseJob = useLibraryMutation(
		['jobs.pause'],
		toastErrorSuccess('failed to pause job', 'job has been paused')
	);
	const cancelJob = useLibraryMutation(
		['jobs.cancel'],
		toastErrorSuccess('failed to cancel job', 'job has been canceled')
	);

	const isJobPaused = useMemo(
		() => group.jobs.some((job) => job.status === 'Paused'),
		[group.jobs]
	);

	const clearJobHandler = () => {
		group.jobs.forEach((job) => {
			clearJob.mutate(job.id);
			//only one toast for all jobs
			if (job.id === group.id) toast.success('Job has been removed');
		});
	};

	return (
		<>
			{/* Resume */}
			{(group.status === 'Queued' || group.status === 'Paused' || isJobPaused) && (
				<Button
					style={tw`h-7 w-7`}
					variant="outline"
					size="sm"
					onPress={() =>
						resumeJob.mutate(
							group.running_job_id != null ? group.running_job_id : group.id
						)
					}
				>
					<Play size={16} color="white" />
				</Button>
			)}
			{/* TODO: This should remove the job from panel */}
			{activeJob !== undefined ? (
				<View style={tw`flex flex-row gap-2`}>
					<Button
						style={tw`h-7 w-7`}
						variant="outline"
						size="sm"
						onPress={() =>
							pauseJob.mutate(
								group.running_job_id != null ? group.running_job_id : group.id
							)
						}
					>
						<Pause size={16} color="white" />
					</Button>
					<Button
						style={tw`h-7 w-7`}
						variant="outline"
						size="sm"
						onPress={() =>
							cancelJob.mutate(
								group.running_job_id != null ? group.running_job_id : group.id
							)
						}
					>
						<Stop size={16} color="white" />
					</Button>
				</View>
			) : (
				<Menu
					containerStyle={tw`max-w-25`}
					trigger={
						<View
							style={tw`flex h-7 w-7 flex-row items-center justify-center rounded-md border border-app-inputborder`}
						>
							<DotsThreeVertical size={16} color="white" />
						</View>
					}
				>
					<MenuItem
						style={twStyle(
							showChildJobs ? 'rounded bg-app-screen/50' : 'bg-transparent'
						)}
						onSelect={() => setShowChildJobs(!showChildJobs)}
						text="Expand"
						icon={Eye}
					/>
					<MenuItem onSelect={clearJobHandler} text="Remove" icon={Trash} />
				</Menu>
			)}
		</>
	);
}
