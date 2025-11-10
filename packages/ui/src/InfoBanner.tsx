import { cva, type VariantProps } from "class-variance-authority";
import { ReactNode } from "react";

const infoBannerStyles = cva(
  "relative mb-6 overflow-hidden rounded-xl border-2",
  {
    variants: {
      variant: {
        info: "border-accent/30 bg-accent/10",
        gray: "border-app-line/20 bg-app-box/10",
        warning: "border-yellow-500/30 bg-yellow-500/10",
        danger: "border-red-500/30 bg-red-500/10",
      },
    },
    defaultVariants: {
      variant: "info",
    },
  },
);

const infoBannerIconStyles = cva("mt-0.5", {
  variants: {
    variant: {
      info: "text-accent",
      gray: "text-ink-dull",
      warning: "text-yellow-500",
      danger: "text-red-500",
    },
  },
  defaultVariants: {
    variant: "info",
  },
});

interface InfoBannerProps extends VariantProps<typeof infoBannerStyles> {
  icon: ReactNode;
  children: ReactNode;
}

export function InfoBanner({ icon, children, variant }: InfoBannerProps) {
  return (
    <div className={infoBannerStyles({ variant })}>
      <div className="p-4">
        <div className="flex items-start gap-3">
          <div className={infoBannerIconStyles({ variant })}>{icon}</div>
          <div>{children}</div>
        </div>
      </div>
    </div>
  );
}

export function InfoBannerText({ children }: { children: ReactNode }) {
  return <p className="text-sm text-ink">{children}</p>;
}

export function InfoBannerSubtext({ children }: { children: ReactNode }) {
  return <p className="mt-1 text-xs text-ink-dull">{children}</p>;
}
