import React, { Children, cloneElement, type ReactElement } from "react";
import { Text, View } from "react-native";
import { cn } from "~/utils/cn";
import type { SettingsRowProps } from "./SettingsRow";

interface SettingsGroupProps {
  header?: string;
  footer?: string;
  children: ReactElement<SettingsRowProps> | ReactElement<SettingsRowProps>[];
  className?: string;
}

export function SettingsGroup({
  header,
  footer,
  children,
  className,
}: SettingsGroupProps) {
  const childArray = Children.toArray(children);
  const totalChildren = childArray.length;

  return (
    <View className={cn("mb-6", className)}>
      {/* Header */}
      {header && (
        <Text className="mb-2 px-4 font-semibold text-ink-dull text-xs uppercase tracking-wider">
          {header}
        </Text>
      )}

      {/* Rows container */}
      <View className="overflow-hidden rounded-[32px]">
        {Children.map(children, (child, index) => {
          if (!React.isValidElement(child)) return child;

          return cloneElement(child, {
            isFirst: index === 0,
            isLast: index === totalChildren - 1,
          });
        })}
      </View>

      {/* Footer */}
      {footer && (
        <Text className="mt-2 px-4 text-ink-faint text-xs">{footer}</Text>
      )}
    </View>
  );
}
