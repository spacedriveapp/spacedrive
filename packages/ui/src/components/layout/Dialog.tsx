import * as DialogPrimitive from '@radix-ui/react-dialog';
import { Button } from '@sd/ui';
import React, { ReactNode } from 'react';

export interface DialogProps {
	trigger: ReactNode;
	ctaLabel?: string;
	ctaAction?: () => void;
	title?: string;
	description?: string;
	children: ReactNode;
}

export default function Dialog(props: DialogProps) {
	return (
		<DialogPrimitive.Root>
			<DialogPrimitive.Trigger asChild>{props.trigger}</DialogPrimitive.Trigger>
			<DialogPrimitive.Portal>
				<DialogPrimitive.Overlay className="fixed top-0 dialog-overlay bottom-0 left-0 right-0 z-50 grid overflow-y-auto bg-black bg-opacity-50 rounded-xl place-items-center m-[1px]">
					<DialogPrimitive.Content className="min-w-[300px] max-w-[400px] dialog-content rounded-md bg-gray-650 text-white border border-gray-550 shadow-deep">
						<div className="p-5">
							<DialogPrimitive.Title className="font-bold ">{props.title}</DialogPrimitive.Title>
							<DialogPrimitive.Description className="text-sm text-gray-300">
								{props.description}
							</DialogPrimitive.Description>
							{props.children}
						</div>
						<div className="flex flex-row justify-end px-3 py-3 space-x-2 bg-gray-600 border-t border-gray-550">
							<DialogPrimitive.Close asChild>
								<Button size="sm" variant="gray">
									Close
								</Button>
							</DialogPrimitive.Close>
							<Button onClick={props.ctaAction} size="sm" variant="primary">
								{props.ctaLabel}
							</Button>
						</div>
					</DialogPrimitive.Content>
				</DialogPrimitive.Overlay>
			</DialogPrimitive.Portal>
		</DialogPrimitive.Root>
	);
}
