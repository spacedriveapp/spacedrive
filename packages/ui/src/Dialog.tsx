'use client';

import * as RDialog from '@radix-ui/react-dialog';
import { animated, useTransition } from '@react-spring/web';
import clsx from 'clsx';
import { ReactElement, ReactNode, useEffect } from 'react';
import { FieldValues, UseFormHandleSubmit } from 'react-hook-form';
import { proxy, ref, subscribe, useSnapshot } from 'valtio';

import { Button, Loader } from '../';
import { Form, FormProps } from './forms/Form';

export function createDialogState(open = false) {
	return proxy({
		open
	});
}

export type DialogState = ReturnType<typeof createDialogState>;

export interface DialogOptions {
	onSubmit?(): void;
}

export interface UseDialogProps extends DialogOptions {
	id: number;
}

class DialogManager {
	private idGenerator = 0;
	private state: Record<string, DialogState> = {};

	dialogs: Record<number, React.FC> = proxy({});

	create(dialog: (props: UseDialogProps) => ReactElement, options?: DialogOptions) {
		const id = this.getId();

		this.dialogs[id] = ref(() => dialog({ id, ...options }));
		this.state[id] = createDialogState(true);

		return new Promise<void>((res) => {
			subscribe(this.dialogs, () => {
				if (!this.dialogs[id]) res();
			});
		});
	}

	getId() {
		return ++this.idGenerator;
	}

	getState(id: number) {
		return this.state[id];
	}

	isAnyDialogOpen() {
		return Object.values(this.state).some((s) => s.open);
	}

	remove(id: number) {
		const state = this.getState(id);

		if (!state) {
			console.error(new Error(`Dialog ${id} not registered!`));
		} else if (state.open === false) {
			delete this.dialogs[id];
			delete this.state[id];
		}
	}
}

export const dialogManager = new DialogManager();

/**
 * Component used to detect when its parent dialog unmounts
 */
function Remover({ id }: { id: number }) {
	useEffect(
		() => () => {
			dialogManager.remove(id);
		},
		[id]
	);

	return null;
}

export function useDialog(props: UseDialogProps) {
	const state = dialogManager.getState(props.id);

	if (!state) throw new Error(`Dialog ${props.id} does not exist!`);

	return {
		...props,
		state
	};
}

export function Dialogs() {
	const dialogs = useSnapshot(dialogManager.dialogs);

	return (
		<>
			{Object.entries(dialogs).map(([id, Dialog]) => (
				<Dialog key={id} />
			))}
		</>
	);
}

const AnimatedDialogContent = animated(RDialog.Content);
const AnimatedDialogOverlay = animated(RDialog.Overlay);

export interface DialogProps<S extends FieldValues>
	extends RDialog.DialogProps,
		Omit<FormProps<S>, 'onSubmit'> {
	title?: string;
	dialog: ReturnType<typeof useDialog>;
	loading?: boolean;
	trigger?: ReactNode;
	ctaLabel?: string;
	ctaSecondLabel?: string;
	onSubmit?: ReturnType<UseFormHandleSubmit<S>>;
	onSubmitSecond?: ReturnType<UseFormHandleSubmit<S>>;
	children?: ReactNode;
	ctaDanger?: boolean;
	cancelDanger?: boolean;
	closeLabel?: string;
	cancelLabel?: string;
	cancelBtn?: boolean;
	description?: ReactNode;
	onCancelled?: boolean | (() => void);
	submitDisabled?: boolean;
	transformOrigin?: string;
	buttonsSideContent?: ReactNode;
	invertButtonFocus?: boolean; //this reverses the focus order of submit/cancel buttons
	errorMessageException?: string; //this is to bypass a specific form error message if it starts with a specific string
	formClassName?: string;
	icon?: ReactNode;
	hideButtons?: boolean;
	ignoreClickOutside?: boolean;
}

export function Dialog<S extends FieldValues>({
	form,
	dialog,
	onSubmit,
	onSubmitSecond,
	onCancelled = true,
	invertButtonFocus,
	...props
}: DialogProps<S>) {
	const stateSnap = useSnapshot(dialog.state);
	const transitions = useTransition(stateSnap.open, {
		from: {
			opacity: 0,
			transform: `translateY(20px)`,
			transformOrigin: props.transformOrigin || 'bottom'
		},
		enter: { opacity: 1, transform: `translateY(0px)` },
		leave: { opacity: 0, transform: `translateY(20px)` },
		config: { mass: 0.4, tension: 200, friction: 10, bounce: 0 }
	});

	const setOpen = (v: boolean) => (dialog.state.open = v);

	const cancelButton = (
		<RDialog.Close asChild>
			<Button
				size="sm"
				variant={props.cancelDanger ? 'colored' : 'gray'}
				onClick={typeof onCancelled === 'function' ? onCancelled : undefined}
				className={clsx(props.cancelDanger && 'border-red-500 bg-red-500')}
			>
				{props.cancelLabel || 'Cancel'}
			</Button>
		</RDialog.Close>
	);

	const closeButton = (
		<RDialog.Close asChild>
			<Button
				disabled={props.loading}
				size="sm"
				variant={'gray'}
				onClick={typeof onCancelled === 'function' ? onCancelled : undefined}
			>
				{props.closeLabel || 'Close'}
			</Button>
		</RDialog.Close>
	);
	const disableCheck = props.errorMessageException
		? !form.formState.isValid &&
			!form.formState.errors.root?.serverError?.message?.startsWith(
				props.errorMessageException as string
			)
		: !form.formState.isValid;

	const submitButton = !props.ctaSecondLabel ? (
		<Button
			type="submit"
			size="sm"
			disabled={form.formState.isSubmitting || props.submitDisabled || disableCheck}
			variant={props.ctaDanger ? 'colored' : 'accent'}
			className={clsx(
				props.ctaDanger &&
					'border-red-500 bg-red-500 focus:ring-1 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-app-selected'
			)}
			onClick={async (e: React.MouseEvent<HTMLElement>) => {
				e.preventDefault();
				await onSubmit?.(e);
				dialog.onSubmit?.();
				setOpen(false);
			}}
		>
			{props.ctaLabel}
		</Button>
	) : (
		<div className="flex flex-row gap-x-2">
			<Button
				type="submit"
				size="sm"
				disabled={form.formState.isSubmitting || props.submitDisabled || disableCheck}
				variant={props.ctaDanger ? 'colored' : 'accent'}
				className={clsx(
					props.ctaDanger &&
						'border-red-500 bg-red-500 focus:ring-1 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-app-selected'
				)}
				onClick={async (e: React.MouseEvent<HTMLElement>) => {
					e.preventDefault();
					await onSubmit?.(e);
					dialog.onSubmit?.();
					setOpen(false);
				}}
			>
				{props.ctaLabel}
			</Button>
			<Button
				type="submit"
				size="sm"
				disabled={form.formState.isSubmitting || props.submitDisabled || disableCheck}
				variant={props.ctaDanger ? 'colored' : 'accent'}
				className={clsx(
					props.ctaDanger &&
						'border-primary-500 bg-primary-500 focus:ring-1 focus:ring-primary-500 focus:ring-offset-2 focus:ring-offset-app-selected'
				)}
				onClick={async (e: React.MouseEvent<HTMLElement>) => {
					e.preventDefault();
					await onSubmitSecond?.(e);
					dialog.onSubmit?.();
					setOpen(false);
				}}
			>
				{props.ctaSecondLabel}
			</Button>
		</div>
	);

	return (
		<RDialog.Root open={stateSnap.open} onOpenChange={setOpen}>
			{props.trigger && <RDialog.Trigger asChild>{props.trigger}</RDialog.Trigger>}
			{transitions((styles, show) =>
				show ? (
					<RDialog.Portal forceMount>
						<AnimatedDialogOverlay
							className="fixed inset-0 z-[102] m-px grid place-items-center overflow-y-auto rounded-xl bg-app/50"
							style={{
								opacity: styles.opacity
							}}
						/>

						<AnimatedDialogContent
							className="!pointer-events-none fixed inset-0 z-[103] grid place-items-center overflow-y-auto"
							style={styles}
							onInteractOutside={(e) =>
								props.ignoreClickOutside && e.preventDefault()
							}
						>
							<Form
								form={form}
								onSubmit={async (e) => {
									e?.preventDefault();
									setOpen(false);
								}}
								className={clsx(
									'!pointer-events-auto my-8 min-w-[300px] max-w-[400px] rounded-md',
									'border border-app-line bg-app-box text-ink shadow-app-shade',
									props.formClassName
								)}
							>
								<RDialog.Title className="flex items-center gap-2.5 border-b border-app-line bg-app-input/60 p-3 font-bold">
									{props.icon && props.icon}
									{props.title}
								</RDialog.Title>
								<div className="p-5">
									{props.description && (
										<RDialog.Description className="mb-2 text-sm text-ink-dull">
											{props.description}
										</RDialog.Description>
									)}

									{props.children}
								</div>
								<div
									className={clsx(
										'flex items-center justify-end space-x-2 border-t border-app-line bg-app-input/60 p-3'
									)}
								>
									{form.formState.isSubmitting && <Loader />}
									{props.buttonsSideContent && (
										<div>{props.buttonsSideContent}</div>
									)}
									<div className="grow" />
									{!props.hideButtons && (
										<div
											className={clsx(
												invertButtonFocus ? 'flex-row-reverse' : 'flex-row',
												'flex gap-2'
											)}
										>
											{invertButtonFocus ? (
												<>
													{submitButton}
													{props.cancelBtn && cancelButton}
													{onCancelled && closeButton}
												</>
											) : (
												<>
													{onCancelled && closeButton}
													{props.cancelBtn && cancelButton}
													{submitButton}
												</>
											)}
										</div>
									)}
								</div>
							</Form>
							<Remover id={dialog.id} />
						</AnimatedDialogContent>
					</RDialog.Portal>
				) : null
			)}
		</RDialog.Root>
	);
}
