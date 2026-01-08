import type { FC, ReactNode } from "react";
import { View, type ViewProps } from "react-native";
import { cn } from "~/utils/cn";

interface CardProps extends ViewProps {
  children: ReactNode;
}

export const Card: FC<CardProps> = ({ children, className, ...props }) => {
  return (
    <View
      className={cn(
        "rounded-lg border border-app-divider bg-app-card p-4",
        className
      )}
      {...props}
    >
      {children}
    </View>
  );
};

export const CardHeader: FC<CardProps> = ({
  children,
  className,
  ...props
}) => {
  return (
    <View className={cn("mb-3", className)} {...props}>
      {children}
    </View>
  );
};

export const CardContent: FC<CardProps> = ({
  children,
  className,
  ...props
}) => {
  return (
    <View className={cn("", className)} {...props}>
      {children}
    </View>
  );
};
