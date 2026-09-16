import {FunnelSimple, X} from '@phosphor-icons/react';
import clsx from 'clsx';
import {useLibraryQuery} from '@sd/ts-client';
import {useExplorer} from './context';
import type {SearchScope} from './context';

export function SearchToolbar() {
	const explorer = useExplorer();
	const isSearching = explorer.mode.type === 'search';

	// The containing source, resolved the same way the path bar resolves it.
	// A location is a pin and pins do not define search domains; a source
	// does, and paths.context reports it alias-normalized.
	const {data: pathContext} = useLibraryQuery(
		{
			type: 'paths.context',
			input: {path: explorer.currentPath!}
		},
		{enabled: isSearching && !!explorer.currentPath}
	);

	if (explorer.mode.type !== 'search') {
		return null;
	}

	const {scope} = explorer.mode;
	const source = pathContext?.source ?? null;

	const handleScopeChange = (newScope: SearchScope) => {
		if (explorer.mode.type === 'search') {
			explorer.enterSearchMode(explorer.mode.query, newScope);
		}
	};

	return (
		<div className="flex items-center gap-3 px-4 py-2 border-b border-sidebar-line/30 bg-sidebar-box/10">
			<div className="flex items-center gap-2">
				<span className="text-xs font-medium text-sidebar-inkDull">
					Search in:
				</span>
				<div className="flex items-center gap-1 rounded-lg bg-sidebar-box/30 p-0.5">
					<ScopeButton
						active={scope === 'folder'}
						onClick={() => handleScopeChange('folder')}
					>
						This Folder
					</ScopeButton>
					<ScopeButton
						active={scope === 'source'}
						disabled={!source}
						title={
							source
								? `Search all of ${source.name}`
								: 'No source contains this folder'
						}
						onClick={() => handleScopeChange('source')}
					>
						{source ? source.name : 'Source'}
					</ScopeButton>
					<ScopeButton
						active={scope === 'library'}
						onClick={() => handleScopeChange('library')}
					>
						Library
					</ScopeButton>
				</div>
			</div>

			<div className="h-4 w-px bg-sidebar-line/30" />

			<button
				className={clsx(
					'flex items-center gap-1.5 px-2 py-1 rounded-md',
					'text-xs font-medium text-sidebar-ink',
					'hover:bg-sidebar-selected/40 transition-colors'
				)}
			>
				<FunnelSimple className="size-3.5" weight="bold" />
				Filters
			</button>

			<div className="flex-1" />

			<button
				onClick={explorer.exitSearchMode}
				className={clsx(
					'flex items-center gap-1.5 px-2 py-1 rounded-md',
					'text-xs font-medium text-sidebar-inkDull',
					'hover:bg-sidebar-selected/40 hover:text-sidebar-ink transition-colors'
				)}
			>
				<X className="size-3.5" weight="bold" />
				Clear Search
			</button>
		</div>
	);
}

interface ScopeButtonProps {
	active: boolean;
	onClick: () => void;
	disabled?: boolean;
	title?: string;
	children: React.ReactNode;
}

function ScopeButton({active, onClick, disabled, title, children}: ScopeButtonProps) {
	return (
		<button
			onClick={onClick}
			disabled={disabled}
			title={title}
			className={clsx(
				'max-w-40 truncate px-3 py-1 rounded-md text-xs font-medium transition-all',
				active
					? 'bg-accent text-white shadow-sm'
					: disabled
						? 'text-sidebar-inkFaint cursor-not-allowed'
						: 'text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-selected/30'
			)}
		>
			{children}
		</button>
	);
}
