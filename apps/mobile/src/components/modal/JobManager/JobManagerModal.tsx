import { useQueryClient } from '@tanstack/react-query';
import { forwardRef } from 'react';
import { FlatList, Text, View } from 'react-native';
import { useJobProgress, useLibraryQuery } from '@sd/client';
import JobGroup from '~/components/job/JobGroup';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { tw } from '~/lib/tailwind';

// TODO:
// - When there is no job, make modal height smaller

export const JobManagerModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const queryClient = useQueryClient();

	const jobGroups = useLibraryQuery(['jobs.reports']);
	const progress = useJobProgress(jobGroups.data);

	// const clearAllJobs = useLibraryMutation(['jobs.clearAll'], {
	// 	onError: () => {
	// 		// TODO: Show error toast
	// 	},
	// 	onSuccess: () => {
	// 		queryClient.invalidateQueries(['jobs.reports ']);
	// 	}
	// });

	return (
		<Modal ref={ref} snapPoints={['60']} title="Job Manager" showCloseButton>
			<FlatList
				data={jobGroups.data}
				style={tw`flex-1 p-4`}
				keyExtractor={(i) => i.id}
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
