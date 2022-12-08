import { StoredKey, useLibraryMutation, useLibraryQuery } from '@sd/client';
import { Button, CategoryHeading, SelectOption } from '@sd/ui';
import { useMemo } from 'react';

import { DefaultProps } from '../primitive/types';
import { DummyKey, Key } from './Key';

export type KeyListProps = DefaultProps;

// ideal for going within a select box
export const SelectOptionMountedKeys = (props: { keys: StoredKey[]; mountedUuids: string[] }) => {
	const { keys, mountedUuids } = props;

	const [mountedKeys] = useMemo(
		() => [keys.filter((key) => mountedUuids.includes(key.uuid)) ?? []],
		[keys, mountedUuids]
	);

	return (
		<>
			{[...mountedKeys]?.map((key) => {
				return (
					<SelectOption value={key.uuid}>Key {key.uuid.substring(0, 8).toUpperCase()}</SelectOption>
				);
			})}
		</>
	);
};

// ideal for going within a select box
export const SelectOptionKeys = (props: { keys: StoredKey[]; mountedUuids: string[] }) => {
	const { keys, mountedUuids } = props;

	const [mountedKeys, unmountedKeys] = useMemo(
		() => [
			keys.filter((key) => mountedUuids.includes(key.uuid)) ?? [],
			keys.filter((key) => !mountedUuids.includes(key.uuid)) ?? []
		],
		[keys, mountedUuids]
	);

	return (
		<>
			{[...mountedKeys, ...unmountedKeys]?.map((key) => {
				return (
					<SelectOption value={key.uuid}>Key {key.uuid.substring(0, 8).toUpperCase()}</SelectOption>
				);
			})}
		</>
	);
};

export const ListOfKeys = () => {
	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted']);

	// use a separate route so we get the default key from the key manager, not the database
	// sometimes the key won't be stored in the database
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
						data={{
							id: key.uuid,
							name: `Key ${key.uuid.substring(0, 8).toUpperCase()}`,
							mounted: mountedKeys.includes(key),
							default: defaultKey.data === key.uuid
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
