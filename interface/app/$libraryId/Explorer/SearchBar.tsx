import clsx from 'clsx';
import { ComponentPropsWithRef, forwardRef, useEffect } from 'react';
import { useForm } from 'react-hook-form';
import { Input, Shortcut } from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

interface Props extends ComponentPropsWithRef<'input'> {
	formClassName?: string;
}

export default forwardRef<HTMLInputElement, Props>((props, forwardedRef) => {
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
					reset();
				}
			}
		};
		document.addEventListener('keydown', keyboardSearchFocus);
		return () => {
			document.removeEventListener('keydown', keyboardSearchFocus);
		};
	}, [forwardedRef, reset]);

	return (
		<form
			data-tauri-drag-region
			onSubmit={handleSubmit(() => null)}
			className={`relative flex h-7 ${props.formClassName}`}
		>
			<Input
				ref={(el) => {
					ref(el);
					if (typeof forwardedRef === 'function') forwardedRef(el);
					else if (forwardedRef) forwardedRef.current = el;
				}}
				placeholder="Search"
				className={clsx('w-52 transition-all duration-200 focus-within:w-60', props.className)}
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
});
