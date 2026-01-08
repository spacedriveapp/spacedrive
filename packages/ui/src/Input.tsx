"use client";

import {
  Eye,
  EyeSlash,
  type Icon,
  type IconProps,
  MagnifyingGlass,
} from "@phosphor-icons/react";
import { cva, type VariantProps } from "class-variance-authority";
import clsx from "clsx";
import { createElement, forwardRef, isValidElement, useState } from "react";

import { Button } from "./Button";

export interface InputBaseProps extends VariantProps<typeof inputStyles> {
  icon?: Icon | React.ReactNode;
  iconPosition?: "left" | "right";
  inputElementClassName?: string;
  right?: React.ReactNode;
}

export type InputProps = InputBaseProps &
  Omit<React.ComponentProps<"input">, "size">;

export type TextareaProps = InputBaseProps & React.ComponentProps<"textarea">;

export const inputSizes = {
  xs: "h-[25px]",
  sm: "h-[30px]",
  md: "h-[36px]",
  lg: "h-[42px]",
  xl: "h-[48px]",
};

export const inputStyles = cva(
  [
    "rounded-lg border text-sm leading-4",
    "outline-none transition-all focus-within:ring-2",
    "text-ink",
  ],
  {
    variants: {
      variant: {
        default: [
          "border-app-line border-app-line bg-app-input/50 placeholder-ink-faint active:border-app-line",
        ],
        transparent: [
          "border-transparent bg-transparent placeholder-ink-dull focus-within:bg-transparent",
          "focus-within:border-transparent focus-within:ring-transparent",
        ],
      },
      error: {
        true: "border-red-500 focus-within:border-red-500 focus-within:ring-red-400/30",
      },
      size: inputSizes,
    },
    defaultVariants: {
      variant: "default",
      size: "sm",
    },
  }
);

export const Input = forwardRef<HTMLInputElement, InputProps>(
  (
    {
      variant,
      size,
      right,
      icon,
      iconPosition = "left",
      className,
      error,
      ...props
    },
    ref
  ) => (
    <div
      className={clsx(
        "group flex",
        inputStyles({
          variant,
          size: right && !size ? "md" : size,
          error,
          className,
        })
      )}
    >
      <div
        className={clsx(
          "flex h-full flex-1 overflow-hidden",
          iconPosition === "right" && "flex-row-reverse"
        )}
      >
        {icon && (
          <div
            className={clsx(
              "flex h-full items-center",
              iconPosition === "left" ? "pr-2 pl-[10px]" : "pr-[10px] pl-2"
            )}
          >
            {isValidElement(icon)
              ? icon
              : createElement<IconProps>(icon as Icon, {
                  size: 18,
                  className: "text-gray-350",
                })}
          </div>
        )}

        <input
          autoComplete={props.autoComplete || "off"}
          className={clsx(
            "focus:!ring-0 flex-1 truncate border-none bg-transparent px-3 text-sm outline-none placeholder:text-ink-faint",
            (right || (icon && iconPosition === "right")) && "pr-0",
            icon && iconPosition === "left" && "pl-0",
            size === "xs" && "!py-0",
            props.inputElementClassName
          )}
          onKeyDown={(e) => {
            e.stopPropagation();
          }}
          ref={ref}
          {...props}
        />
      </div>

      {right && (
        <div
          className={clsx(
            "flex h-full min-w-[12px] items-center",
            size === "lg" ? "px-[5px]" : "px-1"
          )}
        >
          {right}
        </div>
      )}
    </div>
  )
);

export const SearchInput = forwardRef<HTMLInputElement, InputProps>(
  (props, ref) => <Input {...props} icon={MagnifyingGlass} ref={ref} />
);

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ size, variant, error, ...props }, ref) => {
    return (
      <textarea
        {...props}
        className={clsx(
          "h-auto p-2",
          inputStyles({ size, variant, error }),
          props.className
        )}
        onKeyDown={(e) => {
          e.stopPropagation();
        }}
        ref={ref}
      />
    );
  }
);

export interface LabelProps
  extends Omit<React.ComponentProps<"label">, "htmlFor"> {
  slug?: string;
}

export function Label({ slug, children, className, ...props }: LabelProps) {
  return (
    <label
      className={clsx("font-bold font-plex text-sm", className)}
      htmlFor={slug}
      {...props}
    >
      {children}
    </label>
  );
}

interface Props extends InputProps {
  buttonClassnames?: string;
}

export const PasswordInput = forwardRef<HTMLInputElement, Props>(
  (props, ref) => {
    const [showPassword, setShowPassword] = useState(false);

    const CurrentEyeIcon = showPassword ? EyeSlash : Eye;

    return (
      <Input
        {...props}
        onKeyDown={(e) => {
          e.stopPropagation();
        }}
        ref={ref}
        right={
          <Button
            className={clsx(props.buttonClassnames)}
            onClick={() => setShowPassword(!showPassword)}
            size="icon"
            tabIndex={0}
          >
            <CurrentEyeIcon className="!pointer-events-none size-4" />
          </Button>
        }
        type={showPassword ? "text" : "password"}
      />
    );
  }
);
