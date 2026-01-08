export function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div className="mb-1 ml-1 font-semibold text-sidebar-inkFaint text-xs uppercase tracking-wider">
        {title}
      </div>
      <div className="space-y-0.5">{children}</div>
    </div>
  );
}
