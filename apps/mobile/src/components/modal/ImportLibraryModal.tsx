import { BottomSheetFlatList } from '@gorhom/bottom-sheet';
import { NavigationProp, useNavigation } from '@react-navigation/native';
import {
	CloudLibrary,
	useBridgeMutation,
	useBridgeQuery,
	useClientContext,
	useRspcContext
} from '@sd/client';
import { forwardRef } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { Modal, ModalRef } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import { tw } from '~/lib/tailwind';
import { RootStackParamList } from '~/navigation';
import { currentLibraryStore } from '~/utils/nav';

import Empty from '../layout/Empty';
import Fade from '../layout/Fade';

const ImportModalLibrary = forwardRef<ModalRef, unknown>((_, ref) => {
	const navigation = useNavigation<NavigationProp<RootStackParamList>>();
	const modalRef = useForwardedRef(ref);

	const { libraries } = useClientContext();

	const cloudLibraries = useBridgeQuery(['cloud.library.list']);
	const cloudLibrariesData = cloudLibraries.data?.filter(
		(cloudLibrary) => !libraries.data?.find((l) => l.uuid === cloudLibrary.uuid)
	);

	return (
		<Modal
			ref={modalRef}
			snapPoints={cloudLibrariesData?.length !== 0 ? ['30', '50'] : ['30']}
			title="Join a Cloud Library"
			showCloseButton
			onDismiss={() => cloudLibraries.refetch()}
		>
			<View style={tw`relative flex-1`}>
				{cloudLibraries.isLoading ? (
					<View style={tw`mt-10 items-center justify-center`}>
						<ActivityIndicator size="small" />
					</View>
				) : (
					<Fade
						width={20}
						height="100%"
						fadeSides="top-bottom"
						orientation="vertical"
						color="bg-app-modal"
					>
						<BottomSheetFlatList
							data={cloudLibrariesData}
							contentContainerStyle={tw`px-4 pb-6 pt-5`}
							ItemSeparatorComponent={() => <View style={tw`h-2`} />}
							ListEmptyComponent={
								<Empty
									icon="Drive"
									style={tw`mt-2 border-0`}
									iconSize={46}
									description="No cloud libraries available to join"
								/>
							}
							keyExtractor={(item) => item.uuid}
							showsVerticalScrollIndicator={false}
							renderItem={({ item }) => (
								<CloudLibraryCard
									data={item}
									navigation={navigation}
									modalRef={modalRef}
								/>
							)}
						/>
					</Fade>
				)}
			</View>
		</Modal>
	);
});

interface Props {
	data: CloudLibrary;
	modalRef: React.RefObject<ModalRef>;
	navigation: NavigationProp<RootStackParamList>;
}

const CloudLibraryCard = ({ data, modalRef, navigation }: Props) => {
	const rspc = useRspcContext().queryClient;
	const joinLibrary = useBridgeMutation(['cloud.library.join']);
	return (
		<View
			key={data.uuid}
			style={tw`flex flex-row items-center justify-between gap-2 rounded-md border border-app-box bg-app p-2`}
		>
			<Text numberOfLines={1} style={tw`max-w-[80%] text-sm font-bold text-ink`}>
				{data.name}
			</Text>
			<Button
				size="sm"
				variant="accent"
				disabled={joinLibrary.isLoading}
				onPress={async () => {
					const library = await joinLibrary.mutateAsync(data.uuid);

					rspc.setQueryData(['library.list'], (libraries: any) => {
						// The invalidation system beat us to it
						if ((libraries || []).find((l: any) => l.uuid === library.uuid))
							return libraries;

						return [...(libraries || []), library];
					});

					currentLibraryStore.id = library.uuid;

					navigation.navigate('Root', {
						screen: 'Home',
						params: {
							screen: 'OverviewStack',
							params: {
								screen: 'Overview'
							}
						}
					});

					modalRef.current?.dismiss();
				}}
			>
				<Text style={tw`text-sm font-medium text-white`}>
					{joinLibrary.isLoading && joinLibrary.variables === data.uuid
						? 'Joining...'
						: 'Join'}
				</Text>
			</Button>
		</View>
	);
};

export default ImportModalLibrary;
