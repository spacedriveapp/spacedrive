/* eslint-disable tailwindcss/classnames-order */
import { Keys } from '@sd/assets/icons';
import { Button, Tooltip } from '@sd/ui';

export function KeyManager() {
	// const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
	// const isSetup = useLibraryQuery(['keys.isSetup']);

	return (
		<div className="flex h-full max-w-[300px] flex-col">
			<div className="flex w-full flex-col items-center p-4">
				<img src={Keys} className="h-14 w-14" />
				<span className="text-lg font-bold">Key Manager</span>
				<span className="mt-2 text-center text-ink-dull">
					Create encryption keys, mount and unmount your keys to see files decrypted on
					the fly.
				</span>
				<Tooltip className="w-full" label="Coming soon!">
					<Button disabled className="mt-4 w-full" variant="accent">
						Set up
					</Button>
				</Tooltip>
			</div>
		</div>
	);
}
