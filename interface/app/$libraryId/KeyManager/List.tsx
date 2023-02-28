import { useMemo, useRef } from 'react';
import { useLibraryQuery } from '@sd/client';
import { SelectOption } from '@sd/ui';
import { DummyKey, Key } from './Key';

// ideal for going within a select box
// can use mounted or unmounted keys, just provide different inputs
export const KeyListSelectOptions = (props: { keys: string[] }) => (
	<>
		{props.keys.map((key) => (
			<SelectOption key={key} value={key}>
				Key {key.substring(0, 8).toUpperCase()}
			</SelectOption>
		))}
	</>
);

export default () => {
	const keys = useLibraryQuery(['keys.list']);
	const mountedUuids = useLibraryQuery(['keys.listMounted']);
	const defaultKey = useLibraryQuery(['keys.getDefault']);

	const mountingQueue = useRef(new Set<string>());

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
			{[...mountedKeys, ...unmountedKeys]?.map((key) => (
				<Key
					key={key.uuid}
					data={{
						id: key.uuid,
						name: `Key ${key.uuid.substring(0, 8).toUpperCase()}`,
						queue: mountingQueue.current,
						mounted: mountedKeys.includes(key),
						default: defaultKey.data === key.uuid,
						memoryOnly: key.memory_only,
						automount: key.automount
						// key stats need including here at some point
					}}
				/>
			))}
		</>
	);
};
