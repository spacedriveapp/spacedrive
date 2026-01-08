import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, useState } from "react";
import { Pressable, TextInput, type TextInputProps, View } from "react-native";
import { cn } from "~/utils/cn";

const input = cva("rounded-lg border bg-app-box text-ink", {
  variants: {
    variant: {
      default: "border-app-line",
      outline: "border-sidebar-line bg-transparent",
      filled: "border-transparent bg-app-button",
    },
    size: {
      sm: "h-10 px-3",
      default: "h-12 px-4",
      lg: "h-14 px-5",
    },
    disabled: {
      true: "opacity-50",
    },
  },
  defaultVariants: {
    variant: "default",
    size: "default",
  },
});

type InputProps = VariantProps<typeof input> & TextInputProps;

export const Input = forwardRef<TextInput, InputProps>(
  ({ variant, size, disabled, className, editable, style, ...props }, ref) => {
    // Get proper font size and line height based on size
    const fontSize = size === "sm" ? 14 : size === "lg" ? 18 : 16;
    const lineHeight = size === "sm" ? 20 : size === "lg" ? 24 : 22;

    return (
      <TextInput
        className={cn(input({ variant, size, disabled }), className)}
        cursorColor="hsl(220, 90%, 56%)"
        editable={editable ?? !disabled} // accent color
        placeholderTextColor="hsl(235, 10%, 55%)" // ink-faint
        ref={ref}
        style={[style, { fontSize, lineHeight }]}
        {...props}
      />
    );
  }
);

Input.displayName = "Input";

// Password input with show/hide toggle
type PasswordInputProps = InputProps & {
  isNewPassword?: boolean;
};

export const PasswordInput = forwardRef<TextInput, PasswordInputProps>(
  (
    {
      variant,
      size,
      disabled,
      isNewPassword = false,
      className,
      style,
      ...props
    },
    ref
  ) => {
    const [showPassword, setShowPassword] = useState(false);

    // Get proper font size and line height based on size
    const fontSize = size === "sm" ? 14 : size === "lg" ? 18 : 16;
    const lineHeight = size === "sm" ? 20 : size === "lg" ? 24 : 22;

    return (
      <View className="relative">
        <TextInput
          autoCapitalize="none"
          autoComplete={isNewPassword ? "password-new" : "password"}
          autoCorrect={false}
          className={cn(input({ variant, size, disabled }), "pr-12", className)}
          cursorColor="hsl(220, 90%, 56%)"
          placeholder="Password"
          placeholderTextColor="hsl(235, 10%, 55%)"
          ref={ref} // accent color
          secureTextEntry={!showPassword} // ink-faint
          style={[style, { fontSize, lineHeight }]}
          textContentType={isNewPassword ? "newPassword" : "password"}
          {...props}
        />
        <Pressable
          className="absolute top-0 right-4 bottom-0 justify-center"
          disabled={disabled}
          onPress={() => setShowPassword((v) => !v)}
        >
          <View className="h-5 w-5 items-center justify-center">
            {showPassword ? (
              <View className="h-0.5 w-4 rotate-45 bg-ink-dull" />
            ) : (
              <View className="h-3 w-3 rounded-full border-2 border-ink-dull" />
            )}
          </View>
        </Pressable>
      </View>
    );
  }
);

PasswordInput.displayName = "PasswordInput";
