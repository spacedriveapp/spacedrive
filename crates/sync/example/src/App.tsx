import clsx from 'clsx';
import { For, JSX, Suspense, createSignal } from 'solid-js';

import { queryClient, rspc } from './rspc';

export function App() {
	const dbs = rspc.createQuery(() => ['dbs']);

	const createDb = rspc.createMutation('createDatabase', {
		onSuccess: () => {
			queryClient.invalidateQueries();
		}
	});
	const removeDbs = rspc.createMutation('removeDatabases', {
		onSuccess: () => queryClient.invalidateQueries()
	});

	return (
		<div class="p-4 space-y-4">
			<div class="space-x-4">
				<Button onClick={() => createDb.mutate(undefined)}>Add Database</Button>
				<Button onClick={() => removeDbs.mutate(undefined)}>Remove Databases</Button>
			</div>
			<ul class="gap-2 flex flex-row flex-wrap">
				<For each={dbs.data}>
					{(id) => (
						<Suspense fallback={null}>
							<DatabaseView id={id} />
						</Suspense>
					)}
				</For>
			</ul>
		</div>
	);
}

interface DatabaseViewProps {
	id: string;
}
const TABS = ['Tags', 'Files', 'File Paths', 'Messages'];

function DatabaseView(props: DatabaseViewProps) {
	const [currentTab, setCurrentTab] = createSignal<typeof TABS[number]>('Tags');

	return (
		<div class="bg-indigo-300 rounded-md min-w-[40rem] flex-1 overflow-hidden">
			<h1 class="p-2 text-xl font-medium">{props.id}</h1>
			<div>
				<nav class="space-x-2">
					<For each={TABS}>
						{(tab) => (
							<button
								class={clsx('px-2 py-1', tab === currentTab() && 'bg-indigo-400')}
								onClick={() => setCurrentTab(tab)}
							>
								{tab}
							</button>
						)}
					</For>
				</nav>
				<div></div>
			</div>
		</div>
	);
}

function Button(props: JSX.ButtonHTMLAttributes<HTMLButtonElement>) {
	return <button {...props} class="bg-blue-500 text-white px-2 py-1 rounded-md" />;
}
