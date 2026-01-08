"use client";

import { Check } from "@phosphor-icons/react";
import * as Checkbox from "@radix-ui/react-checkbox";
import { cva, type VariantProps } from "class-variance-authority";
import clsx from "clsx";
import { type ComponentProps, forwardRef } from "react";

const styles = cva(
  [
    "form-check-input float-left mt-1 mr-2 size-4 appearance-none rounded-sm border border-gray-300 bg-center bg-contain bg-white bg-no-repeat align-top transition duration-200",
    "checked:border-accent checked:bg-accent checked:hover:bg-accent/80 focus:outline-none",
  ],
  { variants: {} }
);

export interface CheckBoxProps
  extends ComponentProps<"input">,
    VariantProps<typeof styles> {}

export const CheckBox = forwardRef<HTMLInputElement, CheckBoxProps>(
  ({ className, ...props }, ref) => (
    <input
      {...props}
      className={styles({ className })}
      ref={ref}
      type="checkbox"
    />
  )
);

export interface RadixCheckboxProps
  extends ComponentProps<typeof Checkbox.Root> {
  label?: string;
  labelClassName?: string;
}

// TODO: Replace above with this, requires refactor of usage
export const RadixCheckbox = ({
  className,
  labelClassName,
  ...props
}: RadixCheckboxProps) => (
  <div className={clsx("flex items-center", className)}>
    <Checkbox.Root
      className="flex size-[15px] shrink-0 items-center justify-center rounded-[4px] border border-gray-300/10 bg-app-selected radix-state-checked:bg-accent"
      id={props.name}
      {...props}
    >
      <Checkbox.Indicator className="text-white">
        <Check size={12} weight="bold" />
      </Checkbox.Indicator>
    </Checkbox.Root>
    {props.label && (
      <label
        className={clsx("ml-2 font-medium text-sm", labelClassName)}
        htmlFor={props.name}
      >
        {props.label}
      </label>
    )}
  </div>
);
