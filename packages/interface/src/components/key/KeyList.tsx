import { useLibraryQuery, useLibraryMutation  } from '@sd/client';
import { Button, CategoryHeading } from '@sd/ui';

import { DefaultProps } from '../primitive/types';
import { Key } from './Key';
import { useMemo } from 'react';

export type KeyListProps = DefaultProps;

export const ListKeys = (noKeysMessage: boolean) => {
	const keys = useLibraryQuery(['keys.list']);
	const mounted_uuids = useLibraryQuery(['keys.listMounted']);

	// use a separate route so we get the default key from the key manager, not the database
	// sometimes the key won't be stored in the database
	const default_key = useLibraryQuery(['keys.getDefault']);

	const [mountedKeys, unmountedKeys] = useMemo(
		() => [keys.data?.filter((key) => mounted_uuids.data?.includes(key.uuid)) ?? [], keys.data?.filter(key => !mounted_uuids.data?.includes(key.uuid)) ?? []],
		[keys, mounted_uuids]
	);

	if(keys.data?.length === 0 && noKeysMessage) {
		return (
			<CategoryHeading>No keys available.</CategoryHeading>
		)
	}

	return (
		<>
		{[...mountedKeys, ...unmountedKeys]?.map((key, index) => {
			return (
				<Key index={index} data={{
					id: key.uuid,
					// could probably do with a better way to number these, maybe something that doesn't change
					name: `Key ${index + 1}`,
					mounted: mountedKeys.includes(key),
					default: default_key.data === key.uuid,
					// key stats need including here at some point
				}} />
			)
		})}
		</>
	)
};

export function KeyList(props: KeyListProps) {
	const unmountAll = useLibraryMutation(['keys.unmountAll']);

	return (
		<div className="flex flex-col h-full max-h-[360px]">
			<div className="p-3 custom-scroll overlay-scroll">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						{ListKeys(true)}
					</div>
				</div>
			</div>
			<div className="flex w-full p-2 border-t border-app-line rounded-b-md">
				<Button size="sm" variant="gray" onClick={() => {
					unmountAll.mutate(null);
				}}>
					Unmount All
				</Button>
				<div className="flex-grow" />
				<Button size="sm" variant="gray">
					Close
				</Button>
			</div>
		</div>
	);
}
