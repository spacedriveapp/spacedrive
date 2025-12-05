import { cva, VariantProps } from "class-variance-authority";
import React, { forwardRef, useState } from "react";
import { TextInputProps, TextInput, View, Pressable } from "react-native";
import { cn } from "~/utils/cn";

const input = cva("rounded-lg border text-ink bg-app-box", {
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
		const fontSize = size === 'sm' ? 14 : size === 'lg' ? 18 : 16;
		const lineHeight = size === 'sm' ? 20 : size === 'lg' ? 24 : 22;

		return (
			<TextInput
				ref={ref}
				editable={editable ?? !disabled}
				cursorColor="hsl(220, 90%, 56%)" // accent color
				placeholderTextColor="hsl(235, 10%, 55%)" // ink-faint
				style={[style, { fontSize, lineHeight }]}
				className={cn(input({ variant, size, disabled }), className)}
				{...props}
			/>
		);
	},
);

Input.displayName = "Input";

// Password input with show/hide toggle
type PasswordInputProps = InputProps & {
	isNewPassword?: boolean;
};

export const PasswordInput = forwardRef<TextInput, PasswordInputProps>(
	({ variant, size, disabled, isNewPassword = false, className, style, ...props }, ref) => {
		const [showPassword, setShowPassword] = useState(false);

		// Get proper font size and line height based on size
		const fontSize = size === 'sm' ? 14 : size === 'lg' ? 18 : 16;
		const lineHeight = size === 'sm' ? 20 : size === 'lg' ? 24 : 22;

		return (
			<View className="relative">
				<TextInput
					ref={ref}
					autoComplete={isNewPassword ? "password-new" : "password"}
					textContentType={isNewPassword ? "newPassword" : "password"}
					placeholder="Password"
					secureTextEntry={!showPassword}
					autoCorrect={false}
					autoCapitalize="none"
					cursorColor="hsl(220, 90%, 56%)" // accent color
					placeholderTextColor="hsl(235, 10%, 55%)" // ink-faint
					style={[style, { fontSize, lineHeight }]}
					className={cn(input({ variant, size, disabled }), "pr-12", className)}
					{...props}
				/>
				<Pressable
					className="absolute right-4 top-0 bottom-0 justify-center"
					onPress={() => setShowPassword((v) => !v)}
					disabled={disabled}
				>
					<View className="h-5 w-5 items-center justify-center">
						{showPassword ? (
							<View className="h-0.5 w-4 bg-ink-dull rotate-45" />
						) : (
							<View className="h-3 w-3 rounded-full border-2 border-ink-dull" />
						)}
					</View>
				</Pressable>
			</View>
		);
	},
);

PasswordInput.displayName = "PasswordInput";
