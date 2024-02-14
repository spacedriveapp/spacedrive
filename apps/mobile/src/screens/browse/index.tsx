import { CheckCircle } from 'phosphor-react-native';
import React from 'react';
import { useLibraryQuery } from '@sd/client';
import { PulseAnimation } from '~/components/animation/lottie';
import BrowseLocations from '~/components/browse/BrowseLocations';
import BrowseTags from '~/components/browse/BrowseTags';
import Categories from '~/components/browse/Categories';
import Jobs from '~/components/browse/Jobs';
import ScreenContainer from '~/components/layout/ScreenContainer';
import { tw } from '~/lib/tailwind';

function JobIcon() {
	const { data: isActive } = useLibraryQuery(['jobs.isActive']);
	return isActive ? (
		<PulseAnimation style={tw`h-[24px] w-[32px]`} speed={1.5} />
	) : (
		<CheckCircle color="white" size={24} />
	);
}

export default function BrowseScreen() {
	return (
		<ScreenContainer>
			<Categories />
			<BrowseLocations />
			<BrowseTags />
			<Jobs />
			{/* <View style={tw`flex-row items-center w-full gap-x-4`}>
					<JobManagerContextProvider>
						<Pressable onPress={() => modalRef.current?.present()}>
							<JobIcon />
						</Pressable>
						<JobManagerModal ref={modalRef} />
					</JobManagerContextProvider>
				</View> */}
		</ScreenContainer>
	);
}
