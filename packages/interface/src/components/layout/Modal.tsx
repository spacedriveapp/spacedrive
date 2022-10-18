import { Transition } from '@headlessui/react';
import { XMarkIcon } from '@heroicons/react/24/solid';
import { ButtonLink } from '@sd/ui';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';

export function Model(
	props: PropsWithChildren<{
		full?: boolean;
	}>
) {
	return (
		<div
			className={clsx('absolute w-screen h-screen z-30', {
				'pointer-events-none hidden': !open
			})}
		>
			<div className="flex w-screen h-screen p-2 pt-12">
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
						className="absolute top-0 left-0 w-screen h-screen bg-white -z-50 rounded-2xl dark:bg-gray-800 bg-opacity-90"
					/>
				</Transition>
				<ButtonLink to="/" variant="gray" className="!px-1.5 absolute top-2 right-2">
					<XMarkIcon className="w-4 h-4" />
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
					<div className="z-30 flex flex-grow mx-auto bg-white rounded-lg shadow-2xl max-w-7xl dark:bg-gray-650 ">
						{props.children}
					</div>
				</Transition>
			</div>
		</div>
	);
}
