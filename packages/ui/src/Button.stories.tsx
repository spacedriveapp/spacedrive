import type { Meta } from "@storybook/react";

import { Button } from "./Button";

const meta: Meta<typeof Button> = {
  title: "Button",
  component: Button,
  argTypes: {},
  parameters: {
    backgrounds: {
      default: "dark",
    },
  },
  args: {
    children: "Button",
  },
};

export default meta;

type ButtonVariant =
  | "accent"
  | "default"
  | "colored"
  | "dotted"
  | "gray"
  | "outline"
  | "subtle";

export const AllVariants = () => {
  const buttonVariants: ButtonVariant[] = [
    "accent",
    "default",
    "colored",
    "dotted",
    "gray",
    "outline",
    "subtle",
  ];
  return (
    <div className="h-screen w-full bg-app p-10">
      <h1 className="font-bold text-[20px] text-white">Buttons</h1>
      <div className="mt-5 mb-6 ml-[90px] flex flex-col gap-8 text-sm">
        <div className="ml-[100px] grid w-full max-w-[850px] grid-cols-9 items-center gap-6">
          {buttonVariants.map((variant) => (
            <p className="text-white/80" key={variant}>
              {variant}
            </p>
          ))}
        </div>
        <div className="grid w-full max-w-[850px] grid-cols-9 items-center gap-6">
          <h1 className="font-bold text-[14px] text-white">Regular</h1>
          {buttonVariants.map((variant) => (
            <Button key={variant} variant={variant}>
              Button
            </Button>
          ))}
        </div>
        <div className="grid w-full max-w-[850px] grid-cols-9 items-center gap-6">
          <h1 className="font-bold text-[14px] text-white">Hovered</h1>
          {buttonVariants.map((variant) => (
            <Button
              className="sb-pseudo--hover"
              key={variant}
              variant={variant}
            >
              Button
            </Button>
          ))}
        </div>

        <div className="grid w-full max-w-[850px] grid-cols-9 items-center gap-6">
          <h1 className="font-bold text-[14px] text-white">Focused</h1>
          {buttonVariants.map((variant) => (
            <Button
              className="sb-pseudo--focus"
              key={variant}
              variant={variant}
            >
              Button
            </Button>
          ))}
        </div>
      </div>
    </div>
  );
};
