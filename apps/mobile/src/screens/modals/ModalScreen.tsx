import { StatusBar } from 'expo-status-bar';
import React, { useEffect, useState } from 'react';
import { Platform, Text, View } from 'react-native';

import { Button } from '../../components/base/Button';
import useCounter from '../../hooks/useCounter';
import tw from '../../lib/tailwind';

export default function ModalScreen() {
	const [start, setStart] = useState(0);
	const [end, setEnd] = useState(1000);

	const value = useCounter({ name: 'test', start, end });

	useEffect(() => {
		console.log('mount');

		return () => {
			console.log('unmount');
		};
	}, []);

	return (
		<View style={tw`flex-1 items-center justify-center`}>
			<Text style={tw`text-xl font-bold`}>Modal</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Text style={tw`font-bold text-white text-lg`}>{value}</Text>
			<View style={tw`my-8 h-1 w-4/5`} />
			<Text style={tw`text-white`}>START: {start}</Text>
			<Text style={tw`text-white`}>END: {end}</Text>
			<Button variant="primary" size="lg" onPress={() => setStart((val) => val + 1)}>
				<Text>Increase Start</Text>
			</Button>
			<Button variant="primary" size="lg" onPress={() => setStart((val) => val - 1)}>
				<Text>Decrease Start</Text>
			</Button>
			<Button variant="primary" size="lg" onPress={() => setEnd((val) => val + 1)}>
				<Text>Increase End</Text>
			</Button>
			<Button variant="primary" size="lg" onPress={() => setEnd((val) => val - 1)}>
				<Text>Decrease End</Text>
			</Button>
			{/* Use a light status bar on iOS to account for the black space above the modal */}
			<StatusBar style={Platform.OS === 'ios' ? 'light' : 'auto'} />
		</View>
	);
}
