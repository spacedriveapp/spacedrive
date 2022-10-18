import { Transition } from '@headlessui/react';
import { PropsWithChildren } from 'react';

export default function SlideUp(props: PropsWithChildren) {
	return (
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
			{props.children}
		</Transition>
	);
}
