import clsx from 'clsx';
import { useEffect, useRef, useState, useTransition } from 'react';
import { useLocation, useNavigate, useResolvedPath } from 'react-router';
import { createSearchParams, useSearchParams } from 'react-router-dom';
import { useKey, useKeys } from 'rooks';
import { useDebouncedCallback } from 'use-debounce';
import { Input, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { getSearchStore } from '~/hooks/useSearchStore';

export const SEARCH_PARAM_KEY = 'search';

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	const [searchParams, setSearchParams] = useSearchParams();
	const navigate = useNavigate();
	const location = useLocation();

	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	// Wrapping param updates in a transition allows us to track whether
	// updating the params triggers a Suspense somewhere else, providing a free
	// loading state!
	const [_isPending, startTransition] = useTransition();

	const searchPath = useResolvedPath('search');

	const [value, setValue] = useState(searchParams.get(SEARCH_PARAM_KEY) || '');

	const updateParams = useDebouncedCallback((value: string) => {
		startTransition(() =>
			setSearchParams((p) => (p.set(SEARCH_PARAM_KEY, value), p), {
				replace: true
			})
		);
	}, 300);

	useEffect(() => {
		if (searchPath.pathname === location.pathname) updateParams(value);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [value]);

	useKeys([os === 'macOS' ? 'Meta' : 'Ctrl', 'f'], () => searchRef.current?.focus());
	useKey('Escape', () => searchRef.current?.blur());

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="w-52 transition-all duration-200 focus-within:w-60"
			size="sm"
			onChange={(e) => setValue(e.target.value)}
			onBlur={() => {
				getSearchStore().isFocused = false;
				if (value === '') {
					setSearchParams((p) => (p.delete(SEARCH_PARAM_KEY), p), { replace: true });
					navigate(-1);
				}
			}}
			onFocus={() => {
				getSearchStore().isFocused = true;
				if (searchPath.pathname !== location.pathname) {
					navigate({
						pathname: 'search',
						search: createSearchParams({ search: value }).toString()
					});
				}
			}}
			value={value}
			right={
				<>
					<div
						className={clsx(
							'pointer-events-none flex h-7 items-center space-x-1 opacity-70 group-focus-within:hidden'
						)}
					>
						{platform === 'browser' ? (
							<Shortcut
								chars="⌘F"
								aria-label={'Press Command-F to focus search bar'}
								className="border-none"
							/>
						) : os === 'macOS' ? (
							<Shortcut
								chars="⌘F"
								aria-label={'Press Command-F to focus search bar'}
								className="border-none"
							/>
						) : (
							<Shortcut
								chars="CTRL+F"
								aria-label={'Press CTRL-F to focus search bar'}
								className="border-none"
							/>
						)}
					</div>
					{/* This indicates whether the search is loading, a spinner could be put here */}
					{/* {_isPending && <div className="h-8 w-8 bg-red-500" />} */}
				</>
			}
		/>
	);
};
