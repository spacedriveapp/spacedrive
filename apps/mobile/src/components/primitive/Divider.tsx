import { View, type ViewProps } from "react-native";
import { cn } from "~/utils/cn";

interface DividerProps extends ViewProps {}

export function Divider({ className, ...props }: DividerProps) {
  return (
    <View
      className={cn("my-2 h-px w-full bg-app-divider", className)}
      {...props}
    />
  );
}
