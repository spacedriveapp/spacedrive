import { StatusBar } from 'expo-status-bar';
import React from 'react';
import { Platform, Text, View } from 'react-native';

import tw from '../../lib/tailwind';

export default function ModalScreen() {
	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`text-xl font-bold`}>Modal</Text>
			<View style={tw`my-8 h-1 w-4/5`} />

			{/* Use a light status bar on iOS to account for the black space above the modal */}
			<StatusBar style={Platform.OS === 'ios' ? 'light' : 'auto'} />
		</View>
	);
}
