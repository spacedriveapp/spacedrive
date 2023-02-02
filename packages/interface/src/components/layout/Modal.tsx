import { Transition } from '@headlessui/react';
import clsx from 'clsx';
import { X } from 'phosphor-react';
import { PropsWithChildren } from 'react';
import { ButtonLink } from '@sd/ui';

export function Model(
	props: PropsWithChildren<{
		full?: boolean;
	}>
) {
	return (
		<div
			className={clsx('absolute z-30 h-screen w-screen', {
				'pointer-events-none hidden': !open
			})}
		>
			<div className="flex h-screen w-screen p-2 pt-12">
				<Transition
					show
					enter="transition duration-150"
					enterFrom="opacity-0"
					enterTo="opacity-100"
					leave="transition duration-200"
					leaveFrom="opacity-100"
					leaveTo="opacity-0"
				>
					<div
						data-tauri-drag-region
						className="absolute top-0 left-0 -z-50 h-screen w-screen rounded-2xl bg-white bg-opacity-90 dark:bg-gray-800"
					/>
				</Transition>
				<ButtonLink to="/" variant="gray" className="absolute top-2 right-2 !px-1.5">
					<X className="h-4 w-4" />
				</ButtonLink>
				<Transition
					show
					className="flex flex-grow"
					appear
					enter="transition ease-in-out-back duration-200"
					enterFrom="opacity-0 translate-y-5"
					enterTo="opacity-100"
					leave="transition duration-200"
					leaveFrom="opacity-100"
					leaveTo="opacity-0"
				>
					<div className="dark:bg-gray-650 z-30 mx-auto flex max-w-7xl flex-grow rounded-lg bg-white shadow-2xl ">
						{props.children}
					</div>
				</Transition>
			</div>
		</div>
	);
}
