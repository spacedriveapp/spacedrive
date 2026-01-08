import type { ReactNode } from "react";
import { Text, View } from "react-native";
import { cn } from "~/utils/cn";

interface InfoPillProps {
  text: string;
  className?: string;
  textClassName?: string;
  icon?: ReactNode;
}

export function InfoPill({
  text,
  className,
  textClassName,
  icon,
}: InfoPillProps) {
  return (
    <View
      className={cn(
        "flex-row items-center rounded-md bg-app-box px-1.5 py-0.5",
        className
      )}
    >
      {icon && <View className="mr-1">{icon}</View>}
      <Text className={cn("font-medium text-ink-dull text-xs", textClassName)}>
        {text}
      </Text>
    </View>
  );
}

export function PlaceholderPill({
  text,
  className,
  textClassName,
  icon,
}: InfoPillProps) {
  return (
    <View
      className={cn(
        "flex-row items-center gap-1 rounded-md border border-app-divider border-dashed bg-transparent px-1.5 py-0.5",
        className
      )}
    >
      {icon && icon}
      <Text className={cn("font-medium text-ink-faint text-xs", textClassName)}>
        {text}
      </Text>
    </View>
  );
}
