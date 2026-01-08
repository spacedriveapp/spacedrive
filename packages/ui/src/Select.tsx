"use client";

import { Check } from "@phosphor-icons/react";
import * as RS from "@radix-ui/react-select";
import { cva, type VariantProps } from "class-variance-authority";
import clsx from "clsx";
import { forwardRef, type PropsWithChildren } from "react";

const ChevronDouble = (props: React.SVGProps<SVGSVGElement>) => (
  <svg fill="none" height="24" viewBox="0 0 24 24" width="24" {...props}>
    <path
      clipRule="evenodd"
      d="M6.29289 14.2929C6.68342 13.9024 7.31658 13.9024 7.70711 14.2929L12 18.5858L16.2929 14.2929C16.6834 13.9024 17.3166 13.9024 17.7071 14.2929C18.0976 14.6834 18.0976 15.3166 17.7071 15.7071L12.7071 20.7071C12.3166 21.0976 11.6834 21.0976 11.2929 20.7071L6.29289 15.7071C5.90237 15.3166 5.90237 14.6834 6.29289 14.2929Z"
      fill="currentColor"
      fillRule="evenodd"
    />
    <path
      clipRule="evenodd"
      d="M6.29289 9.70711C6.68342 10.0976 7.31658 10.0976 7.70711 9.70711L12 5.41421L16.2929 9.70711C16.6834 10.0976 17.3166 10.0976 17.7071 9.70711C18.0976 9.31658 18.0976 8.68342 17.7071 8.29289L12.7071 3.29289C12.3166 2.90237 11.6834 2.90237 11.2929 3.29289L6.29289 8.29289C5.90237 8.68342 5.90237 9.31658 6.29289 9.70711Z"
      fill="currentColor"
      fillRule="evenodd"
    />
  </svg>
);

export const selectStyles = cva(
  [
    "flex items-center justify-between whitespace-nowrap rounded-md border py-0.5 pr-[10px] pl-3 text-sm",
    "shadow-sm outline-none transition-all focus:ring-2",
    "radix-placeholder:text-ink-faint text-ink",
  ],
  {
    variants: {
      variant: {
        default: ["bg-app-input", "border-app-line"],
      },
      size: {
        sm: "h-[25px] font-normal text-xs",
        md: "h-[34px]",
        lg: "h-[38px]",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "sm",
    },
  }
);

export interface SelectProps<TValue extends string = string>
  extends VariantProps<typeof selectStyles> {
  value: TValue;
  onChange: (value: TValue) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
  containerClassName?: string;
}

export const Select = forwardRef(
  <TValue extends string = string>(
    props: PropsWithChildren<SelectProps<TValue>>,
    ref: React.ForwardedRef<HTMLDivElement>
  ) => (
    <div className={props.containerClassName} ref={ref}>
      <RS.Root
        defaultValue={props.value}
        disabled={props.disabled}
        onValueChange={props.onChange}
        value={props.value}
      >
        <RS.Trigger
          className={selectStyles({
            size: props.size,
            className: props.className,
          })}
        >
          <span className="truncate">
            <RS.Value placeholder={props.placeholder} />
          </span>
          <RS.Icon className="ml-2">
            <ChevronDouble className="text-ink-dull" />
          </RS.Icon>
        </RS.Trigger>

        <RS.Portal>
          <RS.Content className="z-[100] rounded-md border border-app-line bg-app-box shadow-2xl shadow-app-shade/20">
            <RS.Viewport className="p-1">{props.children}</RS.Viewport>
          </RS.Content>
        </RS.Portal>
      </RS.Root>
    </div>
  )
) as <TValue extends string = string>(
  props: PropsWithChildren<SelectProps<TValue>> & {
    ref?: React.ForwardedRef<HTMLDivElement>;
  }
) => JSX.Element;

export function SelectOption(
  props: PropsWithChildren<{ value: string; default?: boolean }>
) {
  return (
    <RS.Item
      className={clsx(
        "relative flex h-6 cursor-pointer select-none items-center rounded pr-3 pl-6",
        "radix-highlighted:text-white text-ink text-sm",
        "radix-highlighted:bg-accent radix-disabled:opacity-50 focus:outline-none"
      )}
      defaultChecked={props.default}
      value={props.value}
    >
      <RS.ItemText>{props.children}</RS.ItemText>
      <RS.ItemIndicator className="absolute left-1 inline-flex items-center">
        <Check className="size-4" />
      </RS.ItemIndicator>
    </RS.Item>
  );
}
