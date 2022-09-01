import tw from '@app/lib/tailwind';
import { useCurrentLibrary, useLibraryStore } from '@app/stores/useLibraryStore';
import React, { useEffect } from 'react';
import { Text, View } from 'react-native';
import { ChevronDownIcon } from 'react-native-heroicons/solid';

import { AnimatedHeight } from '../animation/layout';

const DrawerLibraryManager = () => {
	// init libraries
	const { init: initLibraries, switchLibrary } = useLibraryStore();
	const { currentLibrary, libraries, currentLibraryUuid } = useCurrentLibrary();

	useEffect(() => {
		if (libraries && !currentLibraryUuid) initLibraries(libraries);
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [libraries, currentLibraryUuid]);

	return (
		<View>
			<View
				style={tw`flex flex-row justify-between items-center py-2 px-3 w-full bg-gray-500 border-[#333949] bg-opacity-40 shadow-sm rounded`}
			>
				<Text style={tw`text-gray-200 text-sm font-semibold`}>{currentLibrary.config.name}</Text>
				<ChevronDownIcon size={18} style={tw`text-gray-200 ml-2`} />
			</View>
			<AnimatedHeight hide={true}>
				<Text>Helo</Text>
			</AnimatedHeight>
		</View>
	);
};

export default DrawerLibraryManager;
