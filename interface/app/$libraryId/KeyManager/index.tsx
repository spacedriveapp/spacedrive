/* eslint-disable tailwindcss/classnames-order */

import { Keys } from '@sd/assets/icons';
import { Button, Tooltip } from '@sd/ui';


export function KeyManager() {
	// const isUnlocked = useLibraryQuery(['keys.isUnlocked']);
	// const isSetup = useLibraryQuery(['keys.isSetup']);


	return (
		<div className="flex flex-col h-full max-w-[300px]">
			<div className='flex w-full flex-col items-center p-4'>
				<img src={Keys} className='w-14 h-14' />
				<span className='font-bold text-lg'>Key Manager</span>
				<span className='text-ink-dull text-center mt-2'>Create encryption keys, mount and unmount your keys to see files decrypted on the fly.</span>
				<Tooltip className='w-full' label='Coming soon!'>
					<Button disabled className='mt-4 w-full' variant='accent'>Set up</Button>
				</Tooltip>
			</div>
		</div>
	)

}
