import React from 'react';
import { Button, Text, View } from 'react-native';

import { useBridgeMutation, useBridgeQuery } from '../hooks/rspc';
import tw from '../lib/tailwind';

// This is a temporary page for Oscar to develop and test the Spacedrive Core to RN bridge. This will be replaced by a set of type safe hooks in the future.
export default function TempCoreDebug({ navigation, route }: any) {
	const { data: version } = useBridgeQuery(['version']);
	const { data: libraries } = useBridgeQuery(['library.get']);
	const { mutate: createLibrary } = useBridgeMutation('library.create');

	return (
		<View style={tw`flex-1 justify-center`}>
			<Text style={tw`font-bold text-3xl text-white`}>Core Version: {version}</Text>
			<View style={tw`p-10`}>
				<Text style={tw`font-bold text-3xl text-white`}>Libraries:</Text>
				{(libraries ?? []).map((lib) => (
					<Text key={lib.uuid} style={tw`font-bold text-xl text-white`}>
						{lib.config.name}
					</Text>
				))}
			</View>

			<Button title="New Library" onPress={() => createLibrary('Demo')} />
		</View>
	);
}
