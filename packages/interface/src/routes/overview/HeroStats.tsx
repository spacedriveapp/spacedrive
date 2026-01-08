import { CloudArrowUp, Cpu, Files, HardDrives } from "@phosphor-icons/react";
import { motion } from "framer-motion";

interface HeroStatsProps {
  totalStorage: number; // bytes
  usedStorage: number; // bytes
  totalFiles: number;
  locationCount: number;
  tagCount: number;
  deviceCount: number;
  uniqueContentCount: number;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

export function HeroStats({
  totalStorage,
  usedStorage,
  totalFiles,
  locationCount,
  deviceCount,
  uniqueContentCount,
}: HeroStatsProps) {
  const usagePercent =
    totalStorage > 0 ? (usedStorage / totalStorage) * 100 : 0;

  return (
    <div className="rounded-2xl border border-app-line bg-app-box p-8">
      <div className="grid grid-cols-2 gap-8 lg:grid-cols-4">
        {/* Total Storage */}
        <StatCard
          color="from-accent to-cyan-500"
          icon={HardDrives}
          label="Total Storage"
          progress={usagePercent}
          subtitle={
            <>
              <span className="text-accent">{formatBytes(usedStorage)}</span>{" "}
              used
            </>
          }
          value={formatBytes(totalStorage)}
        />

        {/* Files */}
        <StatCard
          color="from-purple-500 to-pink-500"
          icon={Files}
          label="Files Indexed"
          subtitle={`${uniqueContentCount.toLocaleString()} unique files`}
          value={totalFiles.toLocaleString()}
        />

        {/* Devices */}
        <StatCard
          color="from-green-500 to-emerald-500"
          icon={CloudArrowUp}
          label="Connected Devices"
          subtitle={"registered in library"}
          value={deviceCount}
        />

        {/* Storage Health - Future feature */}
        <StatCard
          color="from-purple-500 to-pink-500"
          icon={Cpu}
          label="AI Compute Power"
          subtitle="across all devices"
          value="70 TOPS"
        />
      </div>
    </div>
  );
}

interface StatCardProps {
  icon: React.ElementType;
  label: string;
  value: string | number;
  subtitle: React.ReactNode;
  progress?: number;
  pulse?: boolean;
  color: string;
  badge?: string;
}

function StatCard({
  icon: Icon,
  label,
  value,
  subtitle,
  progress,
  pulse,
  color,
  badge,
}: StatCardProps) {
  return (
    <div className="relative">
      {badge && (
        <div className="absolute -top-2 -right-2 rounded-full border border-sidebar-line bg-sidebar-box px-2 py-0.5 font-medium text-sidebar-ink text-xs">
          {badge}
        </div>
      )}

      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2">
          <div className="rounded-lg bg-sidebar-box p-2">
            <Icon className="size-5 text-sidebar-ink" weight="duotone" />
          </div>
          {pulse && (
            <motion.div
              animate={{
                scale: [1, 1.2, 1],
                opacity: [1, 0.5, 1],
              }}
              className="size-2 rounded-full bg-accent"
              transition={{ duration: 2, repeat: Number.POSITIVE_INFINITY }}
            />
          )}
        </div>

        <div>
          <div className="mb-1 font-bold text-3xl text-ink">{value}</div>
          <div className="mb-1 text-ink-dull text-xs">{label}</div>
          <div className="text-ink-faint text-xs">{subtitle}</div>
        </div>
      </div>
    </div>
  );
}
