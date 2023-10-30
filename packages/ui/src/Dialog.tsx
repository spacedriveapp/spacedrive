'use client';

import * as RDialog from '@radix-ui/react-dialog';
import { animated, useTransition } from '@react-spring/web';
import { iconNames } from '@sd/assets/util';
import clsx from 'clsx';
import { ReactElement, ReactNode, useEffect } from 'react';
import { FieldValues, UseFormHandleSubmit } from 'react-hook-form';
import { proxy, ref, subscribe, useSnapshot } from 'valtio';

import { Button, Loader } from '../';
import { Form, FormProps } from './forms/Form';
import { Icon } from './Icon';

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

	remove(id: number) {
		const state = this.getState(id);

		if (!state) {
			throw new Error(`Dialog ${id} not registered!`);
		}

		if (state.open === false) {
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
	onSubmit?: ReturnType<UseFormHandleSubmit<S>>;
	children?: ReactNode;
	ctaDanger?: boolean;
	closeLabel?: string;
	cancelBtn?: boolean;
	description?: string;
	onCancelled?: boolean | (() => void);
	submitDisabled?: boolean;
	transformOrigin?: string;
	buttonsSideContent?: ReactNode;
	invertButtonFocus?: boolean; //this reverses the focus order of submit/cancel buttons
	errorMessageException?: string; //this is to bypass a specific form error message if it starts with a specific string
	formClassName?: string;
	icon?: keyof typeof iconNames;
	iconTheme?: 'light' | 'dark';
}

export function Dialog<S extends FieldValues>({
	form,
	dialog,
	onSubmit,
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
				variant="gray"
				onClick={typeof onCancelled === 'function' ? onCancelled : undefined}
			>
				Cancel
			</Button>
		</RDialog.Close>
	);

	const closeButton = (
		<RDialog.Close asChild>
			<Button
				disabled={props.loading}
				size="sm"
				variant="gray"
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

	const submitButton = (
		<Button
			type="submit"
			size="sm"
			disabled={form.formState.isSubmitting || props.submitDisabled || disableCheck}
			variant={props.ctaDanger ? 'colored' : 'accent'}
			className={clsx(
				props.ctaDanger &&
					'border-red-500 bg-red-500 focus:ring-1 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-app-selected'
			)}
		>
			{props.ctaLabel}
		</Button>
	);

	return (
		<RDialog.Root open={stateSnap.open} onOpenChange={setOpen}>
			{props.trigger && <RDialog.Trigger asChild>{props.trigger}</RDialog.Trigger>}
			{transitions((styles, show) =>
				show ? (
					<RDialog.Portal forceMount>
						<AnimatedDialogOverlay
							className="z-49 fixed inset-0 m-[1px] grid place-items-center overflow-y-auto rounded-xl bg-app/50"
							style={{
								opacity: styles.opacity
							}}
						/>

						<AnimatedDialogContent
							className="!pointer-events-none fixed inset-0 z-50 grid place-items-center overflow-y-auto"
							style={styles}
						>
							<Form
								form={form}
								onSubmit={async (e) => {
									e?.preventDefault();
									await onSubmit?.(e);
									dialog.onSubmit?.();
									setOpen(false);
								}}
								className={clsx(
									'!pointer-events-auto my-8 min-w-[300px] max-w-[400px] rounded-md',
									'border border-app-line bg-app-box text-ink shadow-app-shade',
									props.formClassName
								)}
							>
								<div className="p-5">
									<RDialog.Title className="mb-3 flex items-center gap-2.5 font-bold">
										{props.icon && (
											<Icon
												theme={props.iconTheme}
												name={props.icon}
												size={28}
											/>
										)}
										{props.title}
									</RDialog.Title>

									{props.description && (
										<RDialog.Description className="mb-2 text-sm text-ink-dull">
											{props.description}
										</RDialog.Description>
									)}

									{props.children}
								</div>
								<div
									className={clsx(
										'flex justify-end space-x-2 border-t border-app-line bg-app-input/50 p-3'
									)}
								>
									{form.formState.isSubmitting && <Loader />}
									{props.buttonsSideContent && (
										<div>{props.buttonsSideContent}</div>
									)}
									<div className="grow" />
									<div
										className={clsx(
											invertButtonFocus ? 'flex-row-reverse' : ' flex-row',
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
