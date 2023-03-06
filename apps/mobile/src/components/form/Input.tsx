import { VariantProps, cva } from 'class-variance-authority';
import { Eye, EyeSlash } from 'phosphor-react-native';
import { useState } from 'react';
import { Pressable, TextInputProps as RNTextInputProps, TextInput, View } from 'react-native';
import { tw, twStyle } from '~/lib/tailwind';

const input = cva(['rounded-md border text-sm leading-tight shadow-sm'], {
	variants: {
		variant: {
			default: 'border-app-line bg-app text-ink'
		},
		size: {
			default: ['py-2', 'px-3'],
			md: ['py-2.5', 'px-3.5']
		}
	},
	defaultVariants: {
		variant: 'default',
		size: 'default'
	}
});

type InputProps = VariantProps<typeof input> & RNTextInputProps;

export const Input = ({ variant, size, ...props }: InputProps) => {
	const { style, ...otherProps } = props;
	return (
		<TextInput
			placeholderTextColor={tw.color('ink-dull')}
			style={twStyle(input({ variant, size }), style as string)}
			{...otherProps}
		/>
	);
};

// Same as above but configured with password props & show/hide password button

type PasswordInputProps = InputProps & {
	isNewPassword?: boolean;
};

export const PasswordInput = ({ variant, size, ...props }: PasswordInputProps) => {
	const { style, isNewPassword = false, ...otherProps } = props;

	const [showPassword, setShowPassword] = useState(false);

	const Icon = showPassword ? EyeSlash : Eye;

	return (
		<View style={tw`relative`}>
			<TextInput
				autoComplete={isNewPassword ? 'password-new' : 'password'}
				textContentType={isNewPassword ? 'newPassword' : 'password'}
				placeholder="Password"
				secureTextEntry={!showPassword}
				autoCorrect={false}
				autoCapitalize="none"
				placeholderTextColor={tw.color('ink-dull')}
				// Do not use margin here, it will break the absolute positioning of the button.
				// Maybe switch to flexbox?
				style={twStyle(input({ variant, size }), style as string)}
				{...otherProps}
			/>
			<Pressable
				style={tw`absolute inset-y-[10px] right-4`}
				onPress={() => setShowPassword((v) => !v)}
			>
				<Icon size={18} color="white" />
			</Pressable>
		</View>
	);
};
