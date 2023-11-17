import * as Dialog from '@radix-ui/react-dialog';
import { useDismissibleNoticeStore } from '~/hooks';

const FdaDialog = () => {
	const { permissions } = useDismissibleNoticeStore();
	return (
		<Dialog.Root open={permissions}>
			<Dialog.Portal>
				<Dialog.Overlay className="fixed inset-0 z-50 bg-app/80 backdrop-blur-sm radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-open:animate-in radix-state-open:fade-in-0" />
				<Dialog.Content className="fixed left-[50%] top-[50%] z-50 w-96 translate-x-[-50%] translate-y-[-50%] overflow-hidden rounded-md border border-app-line bg-app shadow-lg outline-none duration-200 radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-closed:zoom-out-95 radix-state-closed:slide-out-to-left-1/2 radix-state-closed:slide-out-to-top-[48%] radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95 radix-state-open:slide-in-from-left-1/2 radix-state-open:slide-in-from-top-[48%]">
					hello
				</Dialog.Content>
			</Dialog.Portal>
		</Dialog.Root>
	);
};

export default FdaDialog;
