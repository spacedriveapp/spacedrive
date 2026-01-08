"use client";

import * as RDialog from "@radix-ui/react-dialog";
import { animated, useTransition } from "@react-spring/web";
import clsx from "clsx";
import { type ReactElement, type ReactNode, useEffect, useState } from "react";
import type { FieldValues, UseFormHandleSubmit } from "react-hook-form";

import { Button } from "./Button";
import { Form, type FormProps } from "./forms/Form";
import { Loader } from "./Loader";

export interface DialogState {
  open: boolean;
}

export interface DialogOptions {
  onSubmit?(): void;
}

export interface UseDialogProps extends DialogOptions {
  id: number;
}

class DialogManager {
  private idGenerator = 0;
  private listeners = new Map<number, Set<(state: DialogState) => void>>();
  private states = new Map<number, DialogState>();
  private components = new Map<number, React.FC>();

  create(
    dialog: (props: UseDialogProps) => ReactElement,
    options?: DialogOptions
  ) {
    const id = this.getId();

    this.components.set(id, () => dialog({ id, ...options }));
    this.states.set(id, { open: true });
    this.listeners.set(id, new Set());

    this.notifyGlobalListeners();

    return new Promise<void>((res) => {
      const checkInterval = setInterval(() => {
        if (!this.components.has(id)) {
          clearInterval(checkInterval);
          res();
        }
      }, 100);
    });
  }

  getId() {
    return ++this.idGenerator;
  }

  getState(id: number): DialogState | undefined {
    return this.states.get(id);
  }

  setState(id: number, state: Partial<DialogState>) {
    const current = this.states.get(id);
    if (!current) return;

    const newState = { ...current, ...state };
    this.states.set(id, newState);

    const listeners = this.listeners.get(id);
    if (listeners) {
      listeners.forEach((listener) => listener(newState));
    }
  }

  subscribe(id: number, listener: (state: DialogState) => void) {
    const listeners = this.listeners.get(id);
    if (listeners) {
      listeners.add(listener);
    }

    return () => {
      const listeners = this.listeners.get(id);
      if (listeners) {
        listeners.delete(listener);
      }
    };
  }

  private globalListeners = new Set<() => void>();

  subscribeGlobal(listener: () => void) {
    this.globalListeners.add(listener);
    return () => {
      this.globalListeners.delete(listener);
    };
  }

  private notifyGlobalListeners() {
    this.globalListeners.forEach((listener) => listener());
  }

  getComponents() {
    return Array.from(this.components.entries());
  }

  isAnyDialogOpen() {
    return Array.from(this.states.values()).some((s) => s.open);
  }

  remove(id: number) {
    const state = this.getState(id);

    if (!state) {
      console.error(new Error(`Dialog ${id} not registered!`));
    } else if (state.open === false) {
      this.components.delete(id);
      this.states.delete(id);
      this.listeners.delete(id);
      this.notifyGlobalListeners();
    }
  }
}

export const dialogManager = new DialogManager();

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
  const [state, setState] = useState<DialogState>(() => {
    const initialState = dialogManager.getState(props.id);
    if (!initialState) throw new Error(`Dialog ${props.id} does not exist!`);
    return initialState;
  });

  useEffect(() => {
    return dialogManager.subscribe(props.id, setState);
  }, [props.id]);

  return {
    ...props,
    state,
  };
}

export function Dialogs() {
  const [, forceUpdate] = useState({});

  useEffect(() => {
    return dialogManager.subscribeGlobal(() => {
      forceUpdate({});
    });
  }, []);

  const dialogs = dialogManager.getComponents();

  return (
    <>
      {dialogs.map(([id, Dialog]) => (
        <Dialog key={id} />
      ))}
    </>
  );
}

const AnimatedDialogContent = animated(RDialog.Content);
const AnimatedDialogOverlay = animated(RDialog.Overlay);

export interface DialogProps<S extends FieldValues>
  extends RDialog.DialogProps,
    Omit<FormProps<S>, "onSubmit"> {
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
  invertButtonFocus?: boolean;
  errorMessageException?: string;
  formClassName?: string;
  icon?: ReactNode;
  hideButtons?: boolean;
  hideHeader?: boolean;
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
  const transitions = useTransition(dialog.state.open, {
    from: {
      opacity: 0,
      transform: "translateY(20px)",
      transformOrigin: props.transformOrigin || "bottom",
    },
    enter: { opacity: 1, transform: "translateY(0px)" },
    leave: { opacity: 0, transform: "translateY(20px)" },
    config: { mass: 0.4, tension: 200, friction: 10, bounce: 0 },
  });

  const setOpen = (v: boolean) =>
    dialogManager.setState(dialog.id, { open: v });

  const cancelButton = (
    <RDialog.Close asChild>
      <Button
        className={clsx(props.cancelDanger && "border-red-500 bg-red-500")}
        onClick={typeof onCancelled === "function" ? onCancelled : undefined}
        size="sm"
        variant={props.cancelDanger ? "colored" : "gray"}
      >
        {props.cancelLabel || "Cancel"}
      </Button>
    </RDialog.Close>
  );

  const closeButton = (
    <RDialog.Close asChild>
      <Button
        disabled={props.loading}
        onClick={typeof onCancelled === "function" ? onCancelled : undefined}
        size="sm"
        variant="gray"
      >
        {props.closeLabel || "Close"}
      </Button>
    </RDialog.Close>
  );

  const disableCheck = props.errorMessageException
    ? !(
        form.formState.isValid ||
        form.formState.errors.root?.serverError?.message?.startsWith(
          props.errorMessageException as string
        )
      )
    : !form.formState.isValid;

  const submitButton = props.ctaLabel ? (
    props.ctaSecondLabel ? (
      <div className="flex flex-row gap-x-2">
        <Button
          className={clsx(props.ctaDanger && "border-red-500 bg-red-500")}
          disabled={
            form.formState.isSubmitting || props.submitDisabled || disableCheck
          }
          onClick={async (e: React.MouseEvent<HTMLElement>) => {
            e.preventDefault();
            await onSubmit?.(e);
            dialog.onSubmit?.();
            // Note: onSubmit handler should manage dialog.state.open if needed
          }}
          size="sm"
          type="submit"
          variant={props.ctaDanger ? "colored" : "accent"}
        >
          {props.ctaLabel}
        </Button>
        <Button
          disabled={
            form.formState.isSubmitting || props.submitDisabled || disableCheck
          }
          onClick={async (e: React.MouseEvent<HTMLElement>) => {
            e.preventDefault();
            await onSubmitSecond?.(e);
            dialog.onSubmit?.();
            // Note: onSubmit handler should manage dialog.state.open if needed
          }}
          size="sm"
          type="submit"
          variant="accent"
        >
          {props.ctaSecondLabel}
        </Button>
      </div>
    ) : (
      <Button
        disabled={
          form.formState.isSubmitting || props.submitDisabled || disableCheck
        }
        onClick={async (e: React.MouseEvent<HTMLElement>) => {
          e.preventDefault();
          await onSubmit?.(e);
          dialog.onSubmit?.();
          // Note: onSubmit handler should manage dialog.state.open if needed
        }}
        size="sm"
        type="submit"
        // className={clsx(props.ctaDanger && 'border-red-500 bg-red-500')}
        variant={props.ctaDanger ? "colored" : "accent"}
      >
        {props.ctaLabel}
      </Button>
    )
  ) : null;

  return (
    <RDialog.Root onOpenChange={setOpen} open={dialog.state.open}>
      {props.trigger && (
        <RDialog.Trigger asChild>{props.trigger}</RDialog.Trigger>
      )}
      {transitions((styles, show) =>
        show ? (
          <RDialog.Portal forceMount>
            <AnimatedDialogOverlay
              className="fixed inset-0 z-[102] m-px grid place-items-center overflow-y-auto rounded-xl bg-app/50"
              style={{
                opacity: styles.opacity,
              }}
            />

            <AnimatedDialogContent
              className="!pointer-events-none fixed inset-0 z-[103] grid place-items-center overflow-y-auto"
              onInteractOutside={(e) =>
                props.ignoreClickOutside && e.preventDefault()
              }
              style={styles}
            >
              <Form
                className={clsx(
                  "!pointer-events-auto my-8 min-w-[300px] max-w-[400px] rounded-xl",
                  "border border-app-line bg-app-box text-ink shadow-app-shade",
                  props.formClassName
                )}
                form={form}
                onSubmit={async (e) => {
                  e?.preventDefault();
                  // Only close dialog if there's an actual submit handler
                  // This prevents closing on intermediate steps (like picker screens)
                  if (onSubmit) {
                    await onSubmit(e);
                  }
                }}
              >
                {!props.hideHeader && (
                  <RDialog.Title className="flex items-center gap-2.5 border-app-line border-b bg-app-input/60 p-3 font-bold">
                    {props.icon && props.icon}
                    {props.title}
                  </RDialog.Title>
                )}
                <div className="flex-1 overflow-auto p-5">
                  {props.description && (
                    <RDialog.Description className="mb-2 text-ink-dull text-sm">
                      {props.description}
                    </RDialog.Description>
                  )}

                  {props.children}
                </div>
                {(props.buttonsSideContent ||
                  (!props.hideButtons &&
                    (submitButton || props.cancelBtn || onCancelled))) && (
                  <div
                    className={clsx(
                      "flex items-center justify-end space-x-2 border-app-line border-t bg-app-input/60 p-3"
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
                          invertButtonFocus ? "flex-row-reverse" : "flex-row",
                          "flex gap-2"
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
                )}
              </Form>
              <Remover id={dialog.id} />
            </AnimatedDialogContent>
          </RDialog.Portal>
        ) : null
      )}
    </RDialog.Root>
  );
}
