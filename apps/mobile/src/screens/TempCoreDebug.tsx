import React, { useEffect, useState } from 'react';
import { Button, NativeModules, Text, View } from 'react-native';

import tw from '../lib/tailwind';

const { SDCore } = NativeModules;

// This is a temporary page for Oscar to develop and test the Spacedrive Core to RN bridge. This will be replaced by a set of type safe hooks in the future.
export default function TempCoreDebug({ navigation, route }: any) {
	const [version, setVersion] = useState('');
	const [libraries, setLibraries] = useState([]);

	const fetchVersion = () => {
		SDCore.sd_core_msg(
			JSON.stringify({
				operation: 'query',
				key: ['version']
			})
		).then((version) => {
			setVersion(JSON.parse(version).result);
		});
	};

	const fetchLibraries = () => {
		SDCore.sd_core_msg(
			JSON.stringify({
				operation: 'query',
				key: ['library.get']
			})
		).then((data) => {
			setLibraries(JSON.parse(data).result.map((lib) => lib.config.name));
		});
	};

	useEffect(() => {
		fetchVersion();
		fetchLibraries();
	}, [setVersion, setLibraries]);

	return (
		<View style={tw`flex-1 justify-center`}>
			<Text style={tw`font-bold text-3xl text-white`}>Core Version: {version}</Text>
			<View style={tw`p-10`}>
				<Text style={tw`font-bold text-3xl text-white`}>Libraries:</Text>
				{libraries.map((lib) => (
					<Text key={lib} style={tw`font-bold text-xl text-white`}>
						{lib}
					</Text>
				))}
			</View>

			<Button
				title="New Library"
				onPress={() =>
					SDCore.sd_core_msg(
						JSON.stringify({
							operation: 'mutation',
							key: ['library.create', 'Demo']
						})
					).then((data) => {
						console.log(data);
						fetchLibraries();
					})
				}
			/>
			<Button
				title="Fetch Again"
				onPress={() => {
					fetchVersion();
					fetchLibraries();
				}}
			/>
		</View>
	);
}
