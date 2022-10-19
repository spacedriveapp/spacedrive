import { useMutation } from '@tanstack/react-query';
import clsx from 'clsx';
import { Suspense, useState } from 'react';

import { CRDTOperationType, queryClient, rspc } from './utils/rspc';

export function App() {
	const dbs = rspc.useQuery(['dbs']);

	const createDb = rspc.useMutation('createDatabase');
	const removeDbs = rspc.useMutation('removeDatabases');

	return (
		<div className="p-4 space-y-4">
			<div className="space-x-4">
				<button className={ButtonStyles} onClick={() => createDb.mutate('pullOperations')}>
					Add Database
				</button>
				<button className={ButtonStyles} onClick={() => removeDbs.mutate('pullOperations')}>
					Remove Databases
				</button>
			</div>
			<ul className="gap-2 flex flex-row flex-wrap">
				{dbs.data?.map((id) => (
					<Suspense fallback={null} key={id}>
						<DatabaseView id={id} />
					</Suspense>
				))}
			</ul>
		</div>
	);
}

interface DatabaseViewProps {
	id: string;
}
const TABS = ['File Paths', 'Objects', 'Tags', 'Operations'];

function DatabaseView(props: DatabaseViewProps) {
	const [currentTab, setCurrentTab] = useState<typeof TABS[number]>('File Paths');

	const pullOperations = rspc.useMutation('pullOperations');

	return (
		<div className="bg-indigo-300 rounded-md min-w-[40rem] flex-1 overflow-hidden">
			<div className="flex flex-row justify-between items-center mx-2">
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
				<ul>
					{filePaths.data.map((path) => (
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
				<ul>
					{messages.data.map((message) => (
						<li key={message.id} className="space-x-2">
							<span>{message.id}</span>
							<span>{message.timestamp.toString()}</span>
							<span>{messageType(message.typ)}</span>
						</li>
					))}
				</ul>
			)}
		</div>
	);
}

const ButtonStyles = 'bg-blue-500 text-white px-2 py-1 rounded-md';
