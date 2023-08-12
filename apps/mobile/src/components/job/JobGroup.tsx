import dayjs from 'dayjs';
import { DotsThreeVertical, Folder, Pause, Play, Stop } from 'phosphor-react-native';
import { useMemo, useState } from 'react';
import { Pressable, View } from 'react-native';
import {
	JobGroup,
	JobProgressEvent,
	JobReport,
	getJobNiceActionName,
	getTotalTasks,
	useLibraryMutation,
	useTotalElapsedTimeText
} from '@sd/client';
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

	return (
		<>
			<View>
				<Options activeJob={runningJob} group={group} />
			</View>
			{jobs?.length > 1 ? (
				<>
					<Pressable onPress={() => setShowChildJobs((v) => !v)}>
						<JobContainer
							icon={Folder}
							// TODO:
							// containerStyle
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
							{!showChildJobs && runningJob && <>{/* TODO: ProgressBar */}</>}
						</JobContainer>
					</Pressable>
					{showChildJobs && (
						<AnimatedHeight>
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
				jobs[0] && <Job job={jobs[0]} progress={progress[jobs[0]!.id] || null} />
			)}
		</>
	);
}

function Options({ activeJob, group }: { activeJob?: JobReport; group: JobGroup }) {
	const resumeJob = useLibraryMutation(['jobs.resume'], {
		onError: () => {
			// TODO:
		}
	});
	const pauseJob = useLibraryMutation(['jobs.pause'], {
		onError: () => {
			// TODO:
		}
	});
	const cancelJob = useLibraryMutation(['jobs.cancel'], {
		onError: () => {
			// TODO:
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
					<Play color="white" />
				</Button>
			)}
			{activeJob === undefined ? (
				<Button variant="outline" size="sm">
					<DotsThreeVertical color="white" />
				</Button>
			) : (
				<>
					<Button variant="outline" size="sm" onPress={() => pauseJob.mutate(group.id)}>
						<Pause color="white" />
					</Button>
					<Button variant="outline" size="sm" onPress={() => cancelJob.mutate(group.id)}>
						<Stop color="white" />
					</Button>
				</>
			)}
		</>
	);
}
