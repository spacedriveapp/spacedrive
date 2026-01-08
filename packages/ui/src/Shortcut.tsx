import clsx from "clsx";
import type { ComponentProps } from "react";

export interface ShortcutProps extends ComponentProps<"div"> {
  chars: string;
}

export const Shortcut = (props: ShortcutProps) => {
  const { className, chars, ...rest } = props;

  return (
    <kbd
      className={clsx(
        "border border-b-2 px-1",
        "rounded-md font-bold font-ink-dull text-xs",
        "border-app-line dark:border-transparent",
        className
      )}
      {...rest}
    >
      {chars}
    </kbd>
  );
};
