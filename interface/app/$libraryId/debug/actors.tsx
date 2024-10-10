import { inferSubscriptionResult } from '@spacedrive/rspc-client';
import { useMemo, useState } from 'react';
import { Procedures, useLibraryMutation, useLibrarySubscription } from '@sd/client';
import { Button } from '@sd/ui';
import { useRouteTitle } from '~/hooks/useRouteTitle';

// @million-ignore
export const Component = () => {
	useRouteTitle('Actors');

	const [data, setData] = useState<inferSubscriptionResult<Procedures, 'library.actors'>>({});

	useLibrarySubscription(['library.actors'], { onData: setData });

	const sortedData = useMemo(() => {
		const sorted = Object.entries(data).sort(([a], [b]) => a.localeCompare(b));
		return sorted;
	}, [data]);

	return (
		<div className="size-full">
			<table>
				<tr>
					<th>Name</th>
					<th>Running</th>
				</tr>
				{sortedData.map(([name, running]) => (
					<tr key={name}>
						<td className="pl-2 pr-4 text-left">{name}</td>
						<td className="pl-2 pr-4 text-left">
							{running ? 'Running' : 'Not Running'}
						</td>
						<td className="py-1">
							{running ? <StopButton name={name} /> : <StartButton name={name} />}
						</td>
					</tr>
				))}
			</table>
		</div>
	);
};

function StartButton({ name }: { name: string }) {
	const startActor = useLibraryMutation(['library.startActor']);

	return (
		<Button
			variant="accent"
			disabled={startActor.isPending}
			onClick={() => startActor.mutate(name)}
		>
			{startActor.isPending ? 'Starting...' : 'Start'}
		</Button>
	);
}

function StopButton({ name }: { name: string }) {
	const stopActor = useLibraryMutation(['library.stopActor']);

	return (
		<Button
			variant="accent"
			disabled={stopActor.isPending}
			onClick={() => stopActor.mutate(name)}
		>
			{stopActor.isPending ? 'Stopping...' : 'Stop'}
		</Button>
	);
}
