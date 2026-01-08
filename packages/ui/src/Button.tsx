"use client";

import { cva, cx, type VariantProps } from "class-variance-authority";
import clsx from "clsx";
import { type ComponentProps, forwardRef } from "react";
import { Link } from "react-router-dom";

export type ButtonBaseProps = VariantProps<typeof buttonStyles>;

export type ButtonProps = ButtonBaseProps &
  React.ButtonHTMLAttributes<HTMLButtonElement> & {
    href?: undefined;
  };

export type LinkButtonProps = ButtonBaseProps &
  React.AnchorHTMLAttributes<HTMLAnchorElement> & {
    href?: string;
  };

type Button = {
  (props: ButtonProps): JSX.Element;
  (props: LinkButtonProps): JSX.Element;
};

const hasHref = (
  props: ButtonProps | LinkButtonProps
): props is LinkButtonProps => "href" in props;

export const buttonStyles = cva(
  [
    "cursor-default items-center rounded-xl border font-plex font-semibold tracking-wide outline-none transition-colors duration-100",
    "disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-70",
    "cursor-pointer ring-offset-app-box focus:ring-none focus:ring-offset-none",
  ],
  {
    variants: {
      size: {
        icon: "!p-1",
        lg: "px-3 py-1.5 font-medium text-md",
        md: "px-2.5 py-1.5 font-medium text-sm",
        sm: "px-2 py-0.5 font-medium text-sm",
        xs: "px-1.5 py-0.5 font-normal text-xs",
      },
      variant: {
        default: [
          "bg-transparent hover:bg-app-hover active:bg-app-selected",
          "border border-app-line/80 hover:border-app-line active:border-app-line",
        ],
        subtle: [
          "border-transparent hover:border-app-line/50 active:border-app-line active:bg-app-box/30",
        ],
        outline: [
          "border-sidebar-line/60 hover:border-sidebar-line active:border-sidebar-line/30",
        ],
        dotted: [
          "rounded border border-sidebar-line/70 border-dashed text-center font-medium text-ink-faint text-xs transition hover:border-sidebar-line hover:bg-sidebar-selected/5",
        ],
        gray: [
          "bg-app-button text-white hover:bg-app-hover focus:bg-app-selected",
          "border border-app-line/80 hover:border-app-line focus:ring-1 focus:ring-accent",
        ],
        accent: [
          "border-accent bg-accent text-white shadow-app-shade/10 shadow-md hover:brightness-110 focus:outline-none",
          "focus:ring-1 focus:ring-accent focus:ring-offset-2 focus:ring-offset-app-selected",
        ],
        colored: [
          "text-white shadow-sm hover:bg-opacity-90 active:bg-opacity-100",
        ],
        bare: "",
      },
      rounding: {
        none: "rounded-none",
        left: "rounded-r-none rounded-l-md",
        right: "rounded-r-md rounded-l-none",
        both: "rounded-md",
      },
    },
    defaultVariants: {
      size: "sm",
      variant: "default",
    },
  }
);

export const Button = forwardRef<
  HTMLButtonElement | HTMLAnchorElement,
  ButtonProps | LinkButtonProps
>(({ className, ...props }, ref) => {
  className = cx(buttonStyles(props), className);
  return hasHref(props) ? (
    <a
      {...props}
      className={cx(className, "inline-block no-underline")}
      ref={ref as any}
    />
  ) : (
    <button
      type="button"
      {...(props as ButtonProps)}
      className={className}
      ref={ref as any}
    />
  );
});

export const ButtonLink = forwardRef<
  HTMLAnchorElement,
  ButtonBaseProps & ComponentProps<typeof Link>
>(({ className, size, variant, ...props }, ref) => {
  return (
    <Link
      className={buttonStyles({
        size,
        variant,
        className: clsx(
          "no-underline disabled:cursor-not-allowed disabled:opacity-50",
          className
        ),
      })}
      ref={ref}
      {...props}
    />
  );
});
