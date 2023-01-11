import * as DialogPrimitive from '@radix-ui/react-dialog';
import clsx from 'clsx';
import { ReactNode, useEffect } from 'react';
import { FieldValues } from 'react-hook-form';
import { animated, useTransition } from 'react-spring';

import { Button, Loader } from '../';
import { Form, FormProps } from './forms/Form';

export interface DialogProps<S extends FieldValues>
	extends DialogPrimitive.DialogProps,
		FormProps<S> {
	open: boolean;
	setOpen: (open: boolean) => void;
	trigger?: ReactNode;
	ctaLabel?: string;
	ctaDanger?: boolean;
	title?: string;
	description?: string;
	children?: ReactNode;
	transformOrigin?: string;
	loading?: boolean;
	submitDisabled?: boolean;
}

export function Dialog<S extends FieldValues>({
	form,
	onSubmit,
	open,
	setOpen: onOpenChange,
	...props
}: DialogProps<S>) {
	const transitions = useTransition(open, {
		from: {
			opacity: 0,
			transform: `translateY(20px)`,
			transformOrigin: props.transformOrigin || 'bottom'
		},
		enter: { opacity: 1, transform: `translateY(0px)` },
		leave: { opacity: 0, transform: `translateY(20px)` },
		config: { mass: 0.4, tension: 200, friction: 10, bounce: 0 }
	});

	return (
		<DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
			{props.trigger && <DialogPrimitive.Trigger asChild>{props.trigger}</DialogPrimitive.Trigger>}
			{transitions((styles, show) =>
				show ? (
					<DialogPrimitive.Portal forceMount>
						<DialogPrimitive.Overlay asChild forceMount>
							<animated.div
								className="fixed top-0 bottom-0 left-0 right-0 z-49 grid overflow-y-auto bg-app bg-opacity-50 rounded-xl place-items-center m-[1px]"
								style={{
									opacity: styles.opacity
								}}
							/>
						</DialogPrimitive.Overlay>

						<DialogPrimitive.Content asChild forceMount>
							<animated.div
								className="z-50 fixed top-0 bottom-0 left-0 right-0 grid place-items-center !pointer-events-none"
								style={styles}
							>
								<Form
									form={form}
									onSubmit={onSubmit}
									className="min-w-[300px] max-w-[400px] rounded-md bg-app-box border border-app-line text-ink shadow-app-shade !pointer-events-auto"
								>
									<div className="p-5">
										<DialogPrimitive.Title className="mb-2 font-bold">
											{props.title}
										</DialogPrimitive.Title>
										<DialogPrimitive.Description className="text-sm text-ink-dull">
											{props.description}
										</DialogPrimitive.Description>
										{props.children}
									</div>
									<div className="flex flex-row justify-end px-3 py-3 space-x-2 border-t bg-app-selected border-app-line">
										{form.formState.isSubmitting && <Loader />}
										<div className="flex-grow" />
										<DialogPrimitive.Close asChild>
											<Button disabled={props.loading} size="sm" variant="gray">
												Close
											</Button>
										</DialogPrimitive.Close>
										<Button
											type="submit"
											size="sm"
											disabled={form.formState.isSubmitting || props.submitDisabled}
											variant={props.ctaDanger ? 'colored' : 'accent'}
											className={clsx(props.ctaDanger && 'bg-red-500 border-red-500')}
										>
											{props.ctaLabel}
										</Button>
									</div>
								</Form>
							</animated.div>
						</DialogPrimitive.Content>
					</DialogPrimitive.Portal>
				) : null
			)}
		</DialogPrimitive.Root>
	);
}
