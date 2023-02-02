import * as DialogPrimitive from '@radix-ui/react-dialog';
import clsx from 'clsx';
import { ReactElement, ReactNode, useEffect } from 'react';
import { FieldValues } from 'react-hook-form';
import { animated, useTransition } from 'react-spring';
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
			console.log(`Successfully removed state ${id}`);
		} else console.log(`Tried to remove state ${id} but wasn't pending!`);
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
	return {
		...props,
		state: dialogManager.getState(props.id)
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

	const setOpen = (v: boolean) => (dialog.state.open = v);

	return (
		<DialogPrimitive.Root open={stateSnap.open} onOpenChange={setOpen}>
			{props.trigger && <DialogPrimitive.Trigger asChild>{props.trigger}</DialogPrimitive.Trigger>}
			{transitions((styles, show) =>
				show ? (
					<DialogPrimitive.Portal forceMount>
						<DialogPrimitive.Overlay asChild forceMount>
							<animated.div
								className="z-49 bg-app fixed top-0 bottom-0 left-0 right-0 m-[1px] grid place-items-center overflow-y-auto rounded-xl bg-opacity-50"
								style={{
									opacity: styles.opacity
								}}
							/>
						</DialogPrimitive.Overlay>

						<DialogPrimitive.Content asChild forceMount>
							<animated.div
								className="!pointer-events-none fixed top-0 bottom-0 left-0 right-0 z-50 grid place-items-center"
								style={styles}
							>
								<Form
									form={form}
									onSubmit={async (e) => {
										await onSubmit(e);
										dialog.onSubmit?.();
										setOpen(false);
									}}
									className="bg-app-box border-app-line text-ink shadow-app-shade !pointer-events-auto min-w-[300px] max-w-[400px] rounded-md border"
								>
									<div className="p-5">
										<DialogPrimitive.Title className="mb-2 font-bold">
											{props.title}
										</DialogPrimitive.Title>
										<DialogPrimitive.Description className="text-ink-dull text-sm">
											{props.description}
										</DialogPrimitive.Description>
										{props.children}
									</div>
									<div className="bg-app-selected border-app-line flex flex-row justify-end space-x-2 border-t px-3 py-3">
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
											className={clsx(props.ctaDanger && 'border-red-500 bg-red-500')}
										>
											{props.ctaLabel}
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
