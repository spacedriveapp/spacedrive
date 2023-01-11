import { useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, SelectOption } from '@sd/ui';
import { useMemo, useRef } from 'react';

import { DefaultProps } from '../primitive/types';
import { DummyKey, Key } from './Key';

export type KeyListProps = DefaultProps;

// ideal for going within a select box
// can use mounted or unmounted keys, just provide different inputs
export const SelectOptionKeyList = (props: { keys: string[] }) => (
	<>
		{props.keys.map((key) => (
			<SelectOption key={key} value={key}>
				Key {key.substring(0, 8).toUpperCase()}
			</SelectOption>
		))}
	</>
);

const mountingQueue = useRef(new Set<string>());

export const ListOfKeys = () => {
	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted']);
	const defaultKey = useLibraryQuery(['keys.getDefault']);

	const [mountedKeys, unmountedKeys] = useMemo(
		() => [
			keys.data?.filter((key) => mountedUuids.data?.includes(key.uuid)) ?? [],
			keys.data?.filter((key) => !mountedUuids.data?.includes(key.uuid)) ?? []
		],
		[keys, mountedUuids]
	);

	if (keys.data?.length === 0) {
		return <DummyKey text="No keys available" />;
	}

	return (
		<>
			{[...mountedKeys, ...unmountedKeys]?.map((key, index) => {
				return (
					<Key
						index={index}
						key={key.uuid}
						data={{
							id: key.uuid,
							name: `Key ${key.uuid.substring(0, 8).toUpperCase()}`,
							queue: mountingQueue,
							mounted: mountedKeys.includes(key),
							default: defaultKey.data === key.uuid,
							memoryOnly: key.memory_only,
							automount: key.automount
							// key stats need including here at some point
						}}
					/>
				);
			})}
		</>
	);
};

export const KeyList = (props: KeyListProps) => {
	const unmountAll = useLibraryMutation(['keys.unmountAll']);

	return (
		<div className="flex flex-col h-full max-h-[360px]">
			<div className="p-3 custom-scroll overlay-scroll">
				<div className="">
					{/* <CategoryHeading>Mounted keys</CategoryHeading> */}
					<div className="space-y-1.5">
						<ListOfKeys />
					</div>
				</div>
			</div>
			<div className="flex w-full p-2 border-t border-app-line rounded-b-md">
				<Button
					size="sm"
					variant="gray"
					onClick={() => {
						unmountAll.mutate(null);
					}}
				>
					Unmount All
				</Button>
				<div className="flex-grow" />
				<Button size="sm" variant="gray">
					Close
				</Button>
			</div>
		</div>
	);
};
