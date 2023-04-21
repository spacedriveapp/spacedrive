import clsx from 'clsx';
import { ComponentPropsWithRef, useEffect } from 'react';
import { useRef, useState } from 'react';
import { Input, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

interface Props extends ComponentPropsWithRef<'input'> {
	formClassName?: string;
}

export default (props: Props) => {
	const [searchValue, setSearchValue] = useState('');
	const platform = useOperatingSystem(false);
	const os = useOperatingSystem(true);
	const searchRef = useRef<HTMLInputElement>(null);

	useEffect(() => {
		const keyboardSearchFocus = (event: KeyboardEvent) => {
			if (searchRef.current) {
				if ((event.key === 'f' && event.metaKey) || event.ctrlKey) {
					event.preventDefault();
					searchRef.current?.focus();
				} else if (searchRef.current === document.activeElement && event.key === 'Escape') {
					searchRef.current?.blur();
					setSearchValue('');
				}
			}
		};
		document.addEventListener('keydown', keyboardSearchFocus);
		return () => {
			document.removeEventListener('keydown', keyboardSearchFocus);
		};
	}, [searchRef]);

	return (
		<form data-tauri-drag-region className={clsx('relative flex h-7', props.formClassName)}>
			<Input
				ref={searchRef}
				placeholder="Search"
				className={clsx(
					'w-52 transition-all duration-200 focus-within:w-60',
					props.className
				)}
				size="sm"
				onChange={(e) => setSearchValue(e.target.value)}
				onBlur={() => setSearchValue('')}
				value={searchValue}
				right={
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
				}
			/>
		</form>
	);
};
