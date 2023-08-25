import { Folder } from '@sd/assets/icons';
import dayjs from 'dayjs';
import { DotsThreeVertical, Pause, Play, Stop } from 'phosphor-react-native';
import { useMemo, useState } from 'react';
import { Animated, Pressable, View } from 'react-native';
import { Swipeable } from 'react-native-gesture-handler';
import {
	JobGroup,
	JobProgressEvent,
	JobReport,
	getJobNiceActionName,
	getTotalTasks,
	useLibraryMutation,
	useTotalElapsedTimeText
} from '@sd/client';
import { tw } from '~/lib/tailwind';
import { ProgressBar } from '../animation/ProgressBar';
import { AnimatedHeight } from '../animation/layout';
import { Button } from '../primitive/Button';
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
					tw`flex flex-row items-center pr-4`,
					{ transform: [{ translateX: translate }] }
				]}
			>
				<Options activeJob={runningJob} group={group} />
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
						<AnimatedHeight style={tw`mb-4`}>
							{jobs.map((job) => (
								<Job
									isChild={jobs.length > 1}
									key={job.id}
									job={job}
									progress={progress[job.id] ?? null}
								/>
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

function Options({ activeJob, group }: { activeJob?: JobReport; group: JobGroup }) {
	const resumeJob = useLibraryMutation(['jobs.resume'], {
		onError: () => {
			// TODO: Toasts
		}
	});
	const pauseJob = useLibraryMutation(['jobs.pause'], {
		onError: () => {
			// TODO: Toasts
		}
	});
	const cancelJob = useLibraryMutation(['jobs.cancel'], {
		onError: () => {
			// TODO: Toasts
		}
	});

	const isJobPaused = useMemo(
		() => group.jobs.some((job) => job.status === 'Paused'),
		[group.jobs]
	);

	return (
		<>
			{/* Resume */}
			{(group.status === 'Queued' || group.status === 'Paused' || isJobPaused) && (
				<Button variant="outline" size="sm" onPress={() => resumeJob.mutate(group.id)}>
					<Play size={18} color="white" />
				</Button>
			)}
			{/* TODO: This should remove the job from panel */}
			{!activeJob !== undefined ? (
				<Button variant="outline" size="sm">
					<DotsThreeVertical size={16} color="white" />
				</Button>
			) : (
				<View style={tw`flex flex-row gap-2`}>
					<Button variant="outline" size="sm" onPress={() => pauseJob.mutate(group.id)}>
						<Pause size={16} color="white" />
					</Button>
					<Button variant="outline" size="sm" onPress={() => cancelJob.mutate(group.id)}>
						<Stop size={16} color="white" />
					</Button>
				</View>
			)}
		</>
	);
}
