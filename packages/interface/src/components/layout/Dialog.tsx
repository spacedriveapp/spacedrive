import * as DialogPrimitive from '@radix-ui/react-dialog';
import { Button } from '@sd/ui';
import clsx from 'clsx';
import React, { ReactNode } from 'react';

import Loader from '../primitive/Loader';

export interface DialogProps extends DialogPrimitive.DialogProps {
	trigger: ReactNode;
	ctaLabel?: string;
	ctaDanger?: boolean;
	ctaAction?: () => void;
	title?: string;
	description?: string;
	children?: ReactNode;
	loading?: boolean;
	submitDisabled?: boolean;
}

export default function Dialog(props: DialogProps) {
	return (
		<DialogPrimitive.Root open={props.open} onOpenChange={props.onOpenChange}>
			<DialogPrimitive.Trigger asChild>{props.trigger}</DialogPrimitive.Trigger>
			<DialogPrimitive.Portal>
				<DialogPrimitive.Overlay className="fixed top-0 dialog-overlay bottom-0 left-0 right-0 z-50 grid overflow-y-auto bg-black bg-opacity-50 rounded-xl place-items-center m-[1px]">
					<DialogPrimitive.Content className="min-w-[300px] max-w-[400px] dialog-content rounded-md bg-gray-650 text-white border border-gray-550 shadow-deep">
						<div className="p-5">
							<DialogPrimitive.Title className="mb-2 font-bold">
								{props.title}
							</DialogPrimitive.Title>
							<DialogPrimitive.Description className="text-sm text-gray-300">
								{props.description}
							</DialogPrimitive.Description>
							{props.children}
						</div>
						<div className="flex flex-row justify-end px-3 py-3 space-x-2 bg-gray-600 border-t border-gray-550">
							{props.loading && <Loader />}
							<div className="flex-grow" />
							<DialogPrimitive.Close asChild>
								<Button loading={props.loading} disabled={props.loading} size="sm" variant="gray">
									Close
								</Button>
							</DialogPrimitive.Close>
							<Button
								onClick={props.ctaAction}
								size="sm"
								loading={props.loading}
								disabled={props.loading || props.submitDisabled}
								variant={props.ctaDanger ? 'colored' : 'primary'}
								className={clsx(props.ctaDanger && 'bg-red-500 border-red-500')}
							>
								{props.ctaLabel}
							</Button>
						</div>
					</DialogPrimitive.Content>
				</DialogPrimitive.Overlay>
			</DialogPrimitive.Portal>
		</DialogPrimitive.Root>
	);
}
