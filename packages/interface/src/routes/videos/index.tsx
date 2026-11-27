/**
 * Videos: every video in the library, across every source, in the normal
 * explorer. It is a saved search (content kind = video) rather than a page of
 * its own, so every layout, Quick Preview and the inspector's Footage section
 * work here as they do in any folder. Two filters narrow it to videos with a
 * transcript or ones the Videos extension has analyzed; they live in the URL
 * so the narrowed view can be returned to.
 */

import {FilmStrip} from '@phosphor-icons/react';
import type {SearchFilters} from '@sd/ts-client';
import clsx from 'clsx';
import {useEffect, useMemo, useRef} from 'react';
import {useSearchParams} from 'react-router-dom';
import {TopBarItem, TopBarPortal} from '../../TopBar';
import {ExplorerView, useExplorer} from '../explorer';
import {EMPTY_SEARCH_FILTERS} from '../explorer/context';

const VIDEO_FILTERS: SearchFilters = {
	...EMPTY_SEARCH_FILTERS,
	content_types: ['video']
};

const FOOTAGE_FILTERS = [
	{param: 'transcript', label: 'Has transcript'},
	{param: 'analyzed', label: 'Analyzed'}
] as const;

export function VideosView() {
	const [searchParams, setSearchParams] = useSearchParams();
	const transcript = searchParams.has('transcript');
	const analyzed = searchParams.has('analyzed');
	const {enterFilteredMode, exitFilteredMode, setSortBy} = useExplorer();

	useEffect(() => {
		enterFilteredMode(VIDEO_FILTERS, 'Videos', {transcript, analyzed});
	}, [enterFilteredMode, transcript, analyzed]);

	// Newest first, once; setSortBy changes identity with the explorer's
	// derived state, which would otherwise loop.
	const didSetSort = useRef(false);
	useEffect(() => {
		if (didSetSort.current) return;
		didSetSort.current = true;
		setSortBy('modified');
	}, [setSortBy]);

	useEffect(() => () => exitFilteredMode(), [exitFilteredMode]);

	const titleItem = useMemo(() => {
		const toggle = (param: string) =>
			setSearchParams(
				(params) => {
					const next = new URLSearchParams(params);
					if (next.has(param)) next.delete(param);
					else next.set(param, '1');
					return next;
				},
				{replace: true}
			);
		return (
			<div className="flex items-center gap-2">
				<FilmStrip size={18} weight="bold" className="text-ink-dull" />
				<span className="text-ink text-sm font-semibold">Videos</span>
				{FOOTAGE_FILTERS.map(({param, label}) => {
					const active = searchParams.has(param);
					return (
						<button
							key={param}
							type="button"
							aria-pressed={active}
							onClick={() => toggle(param)}
							className={clsx(
								'rounded-full border px-2.5 py-0.5 text-xs font-medium transition-colors',
								active
									? 'border-accent bg-accent/15 text-ink'
									: 'border-app-line text-ink-dull hover:text-ink'
							)}
						>
							{label}
						</button>
					);
				})}
			</div>
		);
	}, [searchParams, setSearchParams]);

	return (
		<>
			<TopBarPortal
				center={
					<TopBarItem id="videos-title" label="Videos" priority="high">
						{titleItem}
					</TopBarItem>
				}
			/>
			<ExplorerView />
		</>
	);
}
