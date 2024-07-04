import { BottomSheetFlatList } from '@gorhom/bottom-sheet';
import { forwardRef, useEffect } from 'react';
import { useJobProgress, useLibraryQuery } from '@sd/client';
import JobGroup from '~/components/job/JobGroup';
import Empty from '~/components/layout/Empty';
import { Modal, ModalRef } from '~/components/layout/Modal';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';

//TODO: Handle data fetching better when modal is opened

export const JobManagerModal = forwardRef<ModalRef, unknown>((_, ref) => {
	// const rspc = useRspcLibraryContext();
	const jobGroups = useLibraryQuery(['jobs.reports']);
	const progress = useJobProgress(jobGroups.data);
	const modalRef = useForwardedRef(ref);

	//TODO: Add clear all jobs button
	// const clearAllJobs = useLibraryMutation(['jobs.clearAll'], {
	// 	onError: () => {
	// 		toast.error('Failed to clear all jobs.');
	// 	},
	// 	onSuccess: () => {
	// 		queryClient.invalidateQueries(['jobs.reports ']);
	// 	}
	// });

	useEffect(() => {
		if (jobGroups.data?.length === 0) {
			modalRef.current?.snapToPosition('20');
		}
	}, [jobGroups, modalRef]);

	return (
		<Modal ref={modalRef} snapPoints={['60']} title="Recent Jobs" showCloseButton>
			<BottomSheetFlatList
				data={jobGroups.data}
				style={tw`flex-1`}
				keyExtractor={(i) => i.id}
				contentContainerStyle={tw`mt-4`}
				renderItem={({ item }) => <JobGroup group={item} progress={progress} />}
				ListEmptyComponent={<Empty style="border-0" description="No jobs." />}
			/>
		</Modal>
	);
});
