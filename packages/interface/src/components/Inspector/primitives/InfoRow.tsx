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
        "flex justify-between items-start gap-3 text-xs",
        className,
      )}
    >
      <span className="text-sidebar-inkDull shrink-0">{label}</span>
      <span
        className={clsx(
          "text-sidebar-ink font-medium text-right truncate",
          mono && "font-mono text-[11px]",
        )}
      >
        {value}
      </span>
    </div>
  );
}