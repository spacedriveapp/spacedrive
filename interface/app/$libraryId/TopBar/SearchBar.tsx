import clsx from 'clsx';
import { useEffect, useTransition } from 'react';
import { useRef } from 'react';
import { useLocation, useNavigate } from 'react-router';
import { useSearchParams } from 'react-router-dom';
import { Input, Shortcut } from '@sd/ui';
import { useLibraryContext } from '~/../packages/client/src';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

export const SEARCH_PARAM_KEY = 'search';

export default () => {
	const searchRef = useRef<HTMLInputElement>(null);

	const [searchParams, setSearchParams] = useSearchParams();
	const navigate = useNavigate();
	const location = useLocation();

	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);

	const { library } = useLibraryContext();

	useEffect(() => {
		const keyboardSearchFocus = (event: KeyboardEvent) => {
			if (!searchRef.current) return;

			if (event.key === 'f' && (event.metaKey || event.ctrlKey)) {
				event.preventDefault();
				searchRef.current?.focus();
			} else if (searchRef.current === document.activeElement && event.key === 'Escape') {
				setSearchParams((p) => (p.delete(SEARCH_PARAM_KEY), p));
				searchRef.current?.blur();
			}
		};

		document.addEventListener('keydown', keyboardSearchFocus);

		return () => {
			document.removeEventListener('keydown', keyboardSearchFocus);
		};
	}, [searchRef]);

	// Wrapping param updates in a transition allows us to track whether
	// updating the params triggers a Suspense somewhere else, providing a free
	// loading state!
	const [isPending, startTransition] = useTransition();

	return (
		<Input
			ref={searchRef}
			placeholder="Search"
			className="w-52 transition-all duration-200 focus-within:w-60"
			size="sm"
			onChange={(e) => {
				startTransition(() =>
					setSearchParams((p) => (p.set(SEARCH_PARAM_KEY, e.target.value), p))
				);
			}}
			onBlur={() => {
				if ((searchParams.get(SEARCH_PARAM_KEY) || '') === '') {
					setSearchParams((p) => (p.delete(SEARCH_PARAM_KEY), p));
					navigate(-1);
				}
			}}
			// TODO: Use relative navigation. Will require refactor of explorer routes
			onFocus={() => {
				if (location.pathname !== `/${library.uuid}/search`)
					navigate(`/${library.uuid}/search`, { replace: true });
			}}
			value={searchParams.get(SEARCH_PARAM_KEY)! || ''}
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
							/>
						) : os === 'macOS' ? (
							<Shortcut
								chars="⌘F"
								aria-label={'Press Command-F to focus search bar'}
							/>
						) : (
							<Shortcut
								chars="CTRL+F"
								aria-label={'Press CTRL-F to focus search bar'}
							/>
						)}
					</div>
					{isPending && <div className="h-8 w-8 bg-red-500" />}
				</>
			}
		/>
	);
};
