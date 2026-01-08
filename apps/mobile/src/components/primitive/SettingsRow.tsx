import type { ReactNode } from "react";
import { Pressable, type PressableProps, Text, View } from "react-native";
import { cn } from "~/utils/cn";

export interface SettingsRowProps extends Omit<PressableProps, "children"> {
  icon?: ReactNode;
  label: string;
  description?: string;
  trailing?: ReactNode;
  isFirst?: boolean;
  isLast?: boolean;
  onPress?: () => void;
}

export function SettingsRow({
  icon,
  label,
  description,
  trailing,
  isFirst,
  isLast,
  onPress,
  className,
  ...props
}: SettingsRowProps) {
  const Component = onPress ? Pressable : View;

  return (
    <>
      <Component
        className={cn(
          "min-h-[56px] flex-row items-center bg-app-box px-6 py-3",
          isFirst && "rounded-t-[32px]",
          isLast && "rounded-b-[32px]",
          onPress && "active:bg-app-hover",
          className
        )}
        onPress={onPress}
        {...props}
      >
        {/* Icon */}
        {icon && <View className="mr-3">{icon}</View>}

        {/* Label & Description */}
        <View className="flex-1">
          <Text className="text-ink text-lg">{label}</Text>
          {description && (
            <Text className="mt-0.5 text-ink-dull text-sm">{description}</Text>
          )}
        </View>

        {/* Trailing accessory */}
        {trailing && <View className="ml-3">{trailing}</View>}
      </Component>

      {/* Divider (not after last item) */}
      {!isLast && (
        <View className="bg-app-box">
          <View
            className="h-px bg-app-line"
            style={{ marginLeft: icon ? 60 : 24 }}
          />
        </View>
      )}
    </>
  );
}
