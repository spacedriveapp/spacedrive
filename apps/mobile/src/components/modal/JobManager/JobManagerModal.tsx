import { useQueryClient } from '@tanstack/react-query';
import { forwardRef, useRef } from 'react';
import { FlatList, Text, View } from 'react-native';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import JobGroup from '~/components/job/JobGroup';
import { Modal, ModalFlatlist, ModalRef, ModalScrollView } from '~/components/layout/Modal';
import { tw } from '~/lib/tailwind';

// TODO:
// - When there is no job, make modal height smaller

export const JobManagerModal = forwardRef<ModalRef, unknown>((_, ref) => {
	const queryClient = useQueryClient();

	const { data: jobs } = useLibraryQuery(['jobs.reports']);

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
				data={jobs}
				style={tw`flex-1 p-4`}
				keyExtractor={(i) => i.id}
				renderItem={({ item }) => <JobGroup data={item} />}
				ListEmptyComponent={
					<View style={tw`flex h-60 items-center justify-center`}>
						<Text style={tw`text-center text-base text-ink-dull`}>No jobs.</Text>
					</View>
				}
			/>
		</Modal>
	);
});
