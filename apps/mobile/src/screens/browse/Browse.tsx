import { CheckCircle } from 'phosphor-react-native';
import React from 'react';
import { useLibraryQuery } from '@sd/client';
import { PulseAnimation } from '~/components/animation/lottie';
import BrowseCategories from '~/components/browse/BrowseCategories';
import BrowseLocations from '~/components/browse/BrowseLocations';
import BrowseTags from '~/components/browse/BrowseTags';
import Jobs from '~/components/browse/Jobs';
import { ModalRef } from '~/components/layout/Modal';
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
	const modalRef = React.useRef<ModalRef>(null);

	return (
		<ScreenContainer>
			<BrowseCategories />
			<BrowseLocations />
			<BrowseTags />
			<Jobs />
		</ScreenContainer>
	);
}
