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
}

export function Dialog<S extends FieldValues>({
	form,
	dialog,
	onSubmit,
	onCancelled = true,
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
								className="!pointer-events-auto my-8 min-w-[300px] max-w-[400px] rounded-md border border-app-line bg-app-box text-ink shadow-app-shade"
							>
								<div className="p-5">
									<RDialog.Title className="mb-2 font-bold">
										{props.title}
									</RDialog.Title>

									{props.description && (
										<RDialog.Description className="mb-2 text-sm text-ink-dull">
											{props.description}
										</RDialog.Description>
									)}

									{props.children}
								</div>
								<div className="flex flex-row justify-end p-3 space-x-2 border-t border-app-line bg-app-selected">
									{form.formState.isSubmitting && <Loader />}
									{props.buttonsSideContent && (
										<div>{props.buttonsSideContent}</div>
									)}
									<div className="grow" />
									{onCancelled && (
										<RDialog.Close asChild>
											<Button
												disabled={props.loading}
												size="sm"
												variant="gray"
												onClick={
													typeof onCancelled === 'function'
														? onCancelled
														: undefined
												}
											>
												{props.closeLabel || 'Close'}
											</Button>
										</RDialog.Close>
									)}
									{props.cancelBtn && (
										<RDialog.Close asChild>
											<Button
												size="sm"
												variant="gray"
												onClick={
													typeof onCancelled === 'function'
														? onCancelled
														: undefined
												}
											>
												Cancel
											</Button>
										</RDialog.Close>
									)}
									<Button
										type="submit"
										size="sm"
										disabled={
											form.formState.isSubmitting || props.submitDisabled
										}
										variant={props.ctaDanger ? 'colored' : 'accent'}
										className={clsx(
											props.ctaDanger && 'border-red-500 bg-red-500'
										)}
									>
										{props.ctaLabel}
									</Button>
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
