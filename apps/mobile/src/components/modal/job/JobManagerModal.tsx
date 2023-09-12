import { forwardRef, useMemo } from 'react';
import { FlatList, Text, View } from 'react-native';
import { useBridgeQuery, useJobProgress } from '@sd/client';

import JobGroup from '~/components/job/JobGroup';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { tw } from '~/lib/tailwind';

// TODO:
// - When there is no job, make modal height smaller
// - Add clear all jobs button

export const JobManagerModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const jobGroupsById = useBridgeQuery(['jobs.reports']);

	// TODO: Currently we're only clustering togheter all job reports from all libraries without any distinction.
	// TODO: We should probably cluster them by library in the job manager UI

	const jobGroups = useMemo(() => {
		if (!jobGroupsById.data) return [];
		return Object.values(jobGroupsById.data).flat();
	}, [jobGroupsById.data]);

	const progress = useJobProgress(jobGroups);

	// const clearAllJobs = useLibraryMutation(['jobs.clearAll'], {
	// 	onError: () => {
	// 		// TODO: Show error toast
	// 	},
	// 	onSuccess: () => {
	// 		queryClient.invalidateQueries(['jobs.reports ']);
	// 	}
	// });

	return (
		<Modal ref={ref} snapPoints={['60']} title="Recent Jobs" showCloseButton>
			<FlatList
				data={jobGroups}
				style={tw`flex-1`}
				keyExtractor={(i) => i.id}
				contentContainerStyle={tw`mt-4`}
				renderItem={({ item }) => <JobGroup group={item} progress={progress} />}
				ListEmptyComponent={
					<View style={tw`flex h-60 items-center justify-center`}>
						<Text style={tw`text-center text-base text-ink-dull`}>No jobs.</Text>
					</View>
				}
			/>
		</Modal>
	);
});
