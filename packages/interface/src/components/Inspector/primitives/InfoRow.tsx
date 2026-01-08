import clsx from "clsx";

interface InfoRowProps {
  label: string;
  value: string | number | React.ReactNode;
  mono?: boolean;
  className?: string;
}

export function InfoRow({ label, value, mono, className }: InfoRowProps) {
  return (
    <div
      className={clsx(
        "flex items-start justify-between gap-3 text-xs",
        className
      )}
    >
      <span className="shrink-0 text-sidebar-inkDull">{label}</span>
      <span
        className={clsx(
          "truncate text-right font-medium text-sidebar-ink",
          mono && "font-mono text-[11px]"
        )}
      >
        {value}
      </span>
    </div>
  );
}
