import { Text, View } from "react-native";

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
    <View className="mb-6 rounded-2xl border border-app-line bg-app-box p-6">
      <View className="flex-row flex-wrap gap-4">
        {/* Total Storage */}
        <StatCard
          label="Total Storage"
          progress={usagePercent}
          subtitle={`${formatBytes(usedStorage)} used`}
          value={formatBytes(totalStorage)}
        />

        {/* Files */}
        <StatCard
          label="Files Indexed"
          subtitle={`${uniqueContentCount.toLocaleString()} unique`}
          value={totalFiles.toLocaleString()}
        />

        {/* Devices */}
        <StatCard
          label="Devices"
          subtitle="connected"
          value={deviceCount.toString()}
        />

        {/* Locations */}
        <StatCard
          label="Locations"
          subtitle="tracked"
          value={locationCount.toString()}
        />
      </View>
    </View>
  );
}

interface StatCardProps {
  label: string;
  value: string | number;
  subtitle: string;
  progress?: number;
}

function StatCard({ label, value, subtitle, progress }: StatCardProps) {
  return (
    <View className="min-w-[140px] flex-1">
      <View className="mb-2">
        <Text className="font-bold text-2xl text-ink">{value}</Text>
        <Text className="mt-0.5 text-ink-dull text-xs">{label}</Text>
        <Text className="text-ink-faint text-xs">{subtitle}</Text>
      </View>
      {progress !== undefined && (
        <View className="h-1.5 overflow-hidden rounded-full bg-app-darkBox">
          <View
            className="h-full rounded-full bg-accent"
            style={{ width: `${Math.min(progress, 100)}%` }}
          />
        </View>
      )}
    </View>
  );
}
