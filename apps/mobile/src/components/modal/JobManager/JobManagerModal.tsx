import { useQueryClient } from '@tanstack/react-query';
import { forwardRef, useRef } from 'react';
import { Text, View } from 'react-native';
import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Modal, ModalRef, ModalScrollView } from '~/components/layout/Modal';
import { tw } from '~/lib/tailwind';

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
			<ModalScrollView style={tw`flex-1 p-4`}>
				<Text>Hello</Text>
			</ModalScrollView>
		</Modal>
	);
});
