import { Info } from '@phosphor-icons/react';
import * as Dialog from '@radix-ui/react-dialog';
import { LocationFda } from '@sd/assets/videos';
import { Button } from '@sd/ui';
import { usePlatform } from '~/util/Platform';

import { getExplorerStore, useExplorerStore } from '../Explorer/store';

export default function FdaDialog() {
	const { showFda } = useExplorerStore();
	const { requestFdaMacos } = usePlatform();
	return (
		<Dialog.Root open={showFda}>
			<Dialog.Portal>
				<Dialog.Overlay className="fixed inset-0 z-50 bg-app/80 backdrop-blur-sm radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-open:animate-in radix-state-open:fade-in-0" />
				<Dialog.Content className="fixed left-[50%] top-[50%] z-50 w-96 translate-x-[-50%] translate-y-[-50%] overflow-hidden rounded-md border border-app-line bg-app shadow-lg outline-none duration-200 radix-state-closed:animate-out radix-state-closed:fade-out-0 radix-state-closed:zoom-out-95 radix-state-closed:slide-out-to-left-1/2 radix-state-closed:slide-out-to-top-[48%] radix-state-open:animate-in radix-state-open:fade-in-0 radix-state-open:zoom-in-95 radix-state-open:slide-in-from-left-1/2 radix-state-open:slide-in-from-top-[48%]">
					<div className="relative flex aspect-video overflow-hidden border-b border-app-line/50 bg-gradient-to-b from-app-darkBox to-app to-80%">
						<video
							className="absolute w-[500px]"
							autoPlay
							loop
							muted
							controls={false}
							src={LocationFda}
						/>
					</div>
					<div className="p-3 pt-0">
						<div className="py-4 text-center">
							<h2 className="text-lg font-semibold text-ink">Full disk access</h2>
							<p className="mt-px text-sm text-ink-dull">
								We need full disk access in order to index your locations.
							</p>
						</div>

						<div className="flex items-center rounded-md border border-app-line bg-app-box px-3 py-2 text-ink-faint">
							<Info size={20} weight="light" className="mr-2.5 shrink-0" />
							<p className="text-sm font-light">
								For the best Spacedrive experience, we highly recommend this.
							</p>
						</div>
						<div className="flex gap-3">
							<Button
								variant="accent"
								className="mt-3 w-full !rounded"
								size="md"
								onClick={() => {
									requestFdaMacos?.();
								}}
							>
								Enable access
							</Button>
							<Button
								variant="gray"
								className="mt-3 w-full !rounded text-ink"
								size="md"
								onClick={() => (getExplorerStore().showFda = false)}
							>
								Cancel
							</Button>
						</div>
					</div>
				</Dialog.Content>
			</Dialog.Portal>
		</Dialog.Root>
	);
}
