import clsx from 'clsx';
import { Suspense, useState } from 'react';
import { tests } from './test';
import { CRDTOperationType, rspc } from './utils/rspc';

export function App() {
	const dbs = rspc.useQuery(['dbs', 'cringe']);

	const operations = rspc.useQuery(['operations', 'cringe']);

	const createDb = rspc.useMutation('createDatabase');
	const removeDbs = rspc.useMutation('removeDatabases');
	const testCreate = rspc.useMutation('testCreate');

	return (
		<div className="flex h-screen w-screen flex-row divide-x divide-gray-300">
			<div className="flex flex-col space-y-2 p-2">
				<div className="space-x-2">
					<button className={ButtonStyles} onClick={() => createDb.mutate('pullOperations')}>
						Add Database
					</button>
					<button className={ButtonStyles} onClick={() => removeDbs.mutate('pullOperations')}>
						Remove Databases
					</button>
					<button className={ButtonStyles} onClick={() => testCreate.mutate('testCreate')}>
						Test Create
					</button>
				</div>
				<ul className="w-full">
					{Object.entries(tests).map(([key, test]) => (
						<li key={key}>
							<button className="bg-green-300 p-2" onClick={() => test.run()}>
								{test.name}
							</button>
						</li>
					))}
				</ul>
			</div>
			<div className="flex-1">
				<ul className="flex flex-row flex-wrap gap-2 p-2">
					{dbs.data?.map((id) => (
						<Suspense fallback={null} key={id}>
							<DatabaseView id={id} />
						</Suspense>
					))}
				</ul>
			</div>
			<div className="flex w-96 flex-col items-stretch p-2">
				<h1 className="text-center text-2xl font-bold">All Operations</h1>
				<ul className="space-y-2">
					{operations.data?.map((op) => (
						<li key={op.id} className="rounded-md bg-indigo-200 p-2">
							<p className="truncate">ID: {op.id}</p>
							<p className="truncate">Timestamp: {op.timestamp.toString()}</p>
							<p className="truncate">Node: {op.node}</p>
						</li>
					))}
				</ul>
			</div>
		</div>
	);
}

interface DatabaseViewProps {
	id: string;
}
const TABS = ['File Paths', 'Objects', 'Tags', 'Operations'];

function DatabaseView(props: DatabaseViewProps) {
	const [currentTab, setCurrentTab] = useState<(typeof TABS)[number]>('Operations');

	const pullOperations = rspc.useMutation('pullOperations');

	return (
		<div className="min-w-[32rem] flex-1 overflow-hidden rounded-md bg-indigo-300">
			<div className="mx-2 flex flex-row items-center justify-between">
				<h1 className="p-2 text-xl font-medium">{props.id}</h1>
				<button className={ButtonStyles} onClick={() => pullOperations.mutate(props.id)}>
					Pull Operations
				</button>
			</div>
			<div>
				<nav className="space-x-2">
					{TABS.map((tab) => (
						<button
							key={tab}
							className={clsx('px-2 py-1', tab === currentTab && 'bg-indigo-400')}
							onClick={() => setCurrentTab(tab)}
						>
							{tab}
						</button>
					))}
				</nav>
				<Suspense>
					{currentTab === 'File Paths' && <FilePathList db={props.id} />}
					{currentTab === 'Operations' && <OperationList db={props.id} />}
				</Suspense>
			</div>
		</div>
	);
}

function FilePathList(props: { db: string }) {
	const createFilePath = rspc.useMutation('file_path.create');
	const filePaths = rspc.useQuery(['file_path.list', props.db]);

	return (
		<div>
			{filePaths.data && (
				<ul className="font-mono">
					{filePaths.data
						.sort((a, b) => a.id.localeCompare(b.id))
						.map((path) => (
							<li key={path.id}>{JSON.stringify(path)}</li>
						))}
				</ul>
			)}
			<button className="text-center" onClick={() => createFilePath.mutate(props.db)}>
				Create
			</button>
		</div>
	);
}

function messageType(msg: CRDTOperationType) {
	if ('items' in msg) {
		return 'Owned';
	} else if ('record_id' in msg) {
		return 'Shared';
	}
}

function OperationList(props: { db: string }) {
	const messages = rspc.useQuery(['message.list', props.db]);

	return (
		<div>
			{messages.data && (
				<table className="border-separate border-spacing-x-4 font-mono">
					{messages.data
						.sort((a, b) => Number(a.timestamp - b.timestamp))
						.map((message) => (
							<tr key={message.id}>
								<td className="border border-transparent">{message.id}</td>
								<td className="border border-transparent">
									{new Date(Number(message.timestamp) / 10000000).toLocaleTimeString()}
								</td>
								<td className="border border-transparent">{messageType(message.typ)}</td>
							</tr>
						))}
				</table>
			)}
		</div>
	);
}

const ButtonStyles = 'bg-blue-500 text-white px-2 py-1 rounded-md';
