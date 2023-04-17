import * as DialogPrimitive from '@radix-ui/react-dialog';
import clsx from 'clsx';
import { ReactElement, ReactNode, useEffect, useMemo, useState } from 'react';
import { FieldValues } from 'react-hook-form';
import { animated, useTransition } from 'react-spring';
import { proxy, ref, subscribe, useSnapshot } from 'valtio';
import { Button, Loader } from '../';
import { Form, FormProps, z } from './forms/Form';

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

export type DialogSteps<T extends FieldValues> = {
	title?: string;
	description?: string;
	skippable?: boolean;
	ctaLabel?: string;
	body: ReactNode;
	schema: z.ZodSchema<Partial<T>>;
}[];

export interface DialogProps<S extends FieldValues>
	extends DialogPrimitive.DialogProps,
		FormProps<S> {
	dialog: ReturnType<typeof useDialog>;
	trigger?: ReactNode;
	ctaLabel?: string;
	ctaDanger?: boolean;
	title?: string;
	description?: string;
	children?: ReactNode;
	transformOrigin?: string;
	loading?: boolean;
	submitDisabled?: boolean;
	steps?: DialogSteps<S>;
}

export function Dialog<S extends FieldValues>({
	form,
	onSubmit,
	dialog,
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

	const [currentStep, setCurrentStep] = useState<number>(0);

	const formValues = form.watch();

	const step = useMemo(() => props.steps?.[currentStep], [currentStep, props.steps]);
	const isStepValid = useMemo(
		() => step?.schema.safeParse(formValues).success,
		[step, formValues]
	);
	const skippable = step?.skippable;

	const setOpen = (v: boolean) => (dialog.state.open = v);

	return (
		<DialogPrimitive.Root
			open={stateSnap.open}
			onOpenChange={(open) => {
				if (form.formState.isSubmitting) return;
				setOpen(open);
			}}
		>
			{props.trigger && (
				<DialogPrimitive.Trigger asChild>{props.trigger}</DialogPrimitive.Trigger>
			)}
			{transitions((styles, show) =>
				show ? (
					<DialogPrimitive.Portal forceMount>
						<DialogPrimitive.Overlay asChild forceMount>
							<animated.div
								className="z-49 fixed inset-0 m-[1px] grid place-items-center overflow-y-auto rounded-xl bg-app/50"
								style={{
									opacity: styles.opacity
								}}
							/>
						</DialogPrimitive.Overlay>

						<DialogPrimitive.Content asChild forceMount>
							<animated.div
								className="!pointer-events-none fixed inset-0 z-50 grid place-items-center"
								style={styles}
							>
								<Form
									form={form}
									onSubmit={async (e) => {
										if (props.steps && currentStep < props.steps.length - 1) {
											e?.preventDefault();
											if (isStepValid) {
												if (form.formState.errors) form.clearErrors();
												setCurrentStep(currentStep + 1);
											}
										} else {
											await onSubmit?.(e);
											dialog.onSubmit?.();
											setOpen(false);
										}
									}}
									className={clsx(
										'!pointer-events-auto min-w-[300px] max-w-[400px] rounded-md border border-app-line bg-app-box text-ink shadow-app-shade',
										props.className
									)}
								>
									<div className="p-5">
										<div className="mb-5">
											<DialogPrimitive.Title className="mb-0.5 font-bold">
												{step?.title || props.title}
											</DialogPrimitive.Title>
											<DialogPrimitive.Description className="text-sm text-ink-dull">
												{step?.description || props.description}
											</DialogPrimitive.Description>
										</div>

										{step?.body || props.children}
									</div>
									<div className="flex flex-row justify-end space-x-2 border-t border-app-line bg-app-selected p-3">
										{form.formState.isSubmitting && <Loader />}
										{currentStep > 0 && !form.formState.isSubmitting && (
											<Button
												type="button"
												onClick={() => setCurrentStep(currentStep - 1)}
												className="border-none hover:underline"
												variant="bare"
											>
												Back
											</Button>
										)}

										<div className="grow" />
										<DialogPrimitive.Close asChild>
											<Button
												disabled={props.loading}
												size="sm"
												variant="gray"
											>
												Close
											</Button>
										</DialogPrimitive.Close>
										<Button
											type="submit"
											size="sm"
											disabled={
												form.formState.isSubmitting || step
													? !isStepValid
													: props.submitDisabled
											}
											variant={
												props.ctaDanger
													? 'colored'
													: skippable
													? 'gray'
													: 'accent'
											}
											className={clsx(
												props.ctaDanger && 'border-red-500 bg-red-500',
												skippable && 'transition-none dark:bg-app-box'
											)}
										>
											{props.steps && currentStep < props.steps.length - 1
												? step?.ctaLabel
													? step.ctaLabel
													: skippable
													? 'Skip'
													: 'Next'
												: props.ctaLabel || 'Submit'}
										</Button>
									</div>
								</Form>
								<Remover id={dialog.id} />
							</animated.div>
						</DialogPrimitive.Content>
					</DialogPrimitive.Portal>
				) : null
			)}
		</DialogPrimitive.Root>
	);
}
