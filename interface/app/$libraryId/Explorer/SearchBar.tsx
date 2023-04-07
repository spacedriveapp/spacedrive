import clsx from 'clsx';
import { ComponentPropsWithRef, forwardRef, useEffect } from 'react';
import { useForm } from 'react-hook-form';
import { Input, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

export default forwardRef<HTMLInputElement, ComponentPropsWithRef<'input'>>(
	(props, forwardedRef) => {
		const {
			register,
			handleSubmit,
			reset,
			formState: { dirtyFields }
		} = useForm();

		const { ref, ...searchField } = register('searchField', {
			onBlur: () => {
				// if there's no text in the search bar, don't mark it as dirty so the key hint shows
				if (!dirtyFields.searchField) reset();
			}
		});

		const platform = useOperatingSystem(false);
		const os = useOperatingSystem(true);

		useEffect(() => {
			const keyboardSearchFocus = (event: KeyboardEvent) => {
				if (typeof forwardedRef !== 'function') {
					if ((event.key === 'f' && event.metaKey) || event.ctrlKey) {
						event.preventDefault();
						forwardedRef?.current?.focus();
					} else if (forwardedRef?.current === document.activeElement && event.key === 'Escape') {
						forwardedRef.current?.blur();
					}
				}
			};
			document.addEventListener('keydown', keyboardSearchFocus);
			return () => {
				document.removeEventListener('keydown', keyboardSearchFocus);
			};
			// eslint-disable-next-line react-hooks/exhaustive-deps
		}, []);

		return (
			<form onSubmit={handleSubmit(() => null)} className="relative flex h-7">
				<Input
					ref={(el) => {
						ref(el);
						if (typeof forwardedRef === 'function') forwardedRef(el);
						else if (forwardedRef) forwardedRef.current = el;
					}}
					placeholder="Search"
					className={clsx('w-32 transition-all focus-within:w-52', props.className)}
					size="sm"
					{...searchField}
					right={
						<div
							className={clsx(
								'pointer-events-none flex h-7 items-center space-x-1 opacity-70 group-focus-within:hidden'
							)}
						>
							{platform === 'browser' ? (
								<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
							) : os === 'macOS' ? (
								<Shortcut chars="⌘F" aria-label={'Press Command-F to focus search bar'} />
							) : (
								<Shortcut chars="CTRL+F" aria-label={'Press CTRL-F to focus search bar'} />
							)}
						</div>
					}
				/>
			</form>
		);
	}
);
