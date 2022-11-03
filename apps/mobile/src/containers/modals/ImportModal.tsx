import { BottomSheetModal } from '@gorhom/bottom-sheet';
import { useLibraryMutation } from '@sd/client';
import * as ML from 'expo-media-library';
import { forwardRef, useCallback } from 'react';
import { Alert, Platform, Text, View } from 'react-native';
import DocumentPicker from 'react-native-document-picker';
// import RFS from 'react-native-fs';
import { Modal } from '~/components/layout/Modal';
import { Button } from '~/components/primitive/Button';
import useForwardedRef from '~/hooks/useForwardedRef';
import tw from '~/lib/tailwind';

const ImportModal = forwardRef<BottomSheetModal, unknown>((_, ref) => {
	const modalRef = useForwardedRef(ref);

	const { mutate: createLocation } = useLibraryMutation('locations.create', {
		onError: (error) => {
			console.error(error);
		},
		onSettled: () => {
			// Close the modal
			modalRef.current?.close();
		}
	});

	const handleFilesButton = useCallback(async () => {
		try {
			const response = await DocumentPicker.pickDirectory({
				presentationStyle: 'pageSheet'
			});

			createLocation({
				path: decodeURIComponent(response.uri.replace('file://', '')),
				indexer_rules_ids: []
			});
		} catch (err) {
			console.error(err);
		}
	}, [createLocation]);

	// Temporary until we decide on the user flow
	const handlePhotosButton = useCallback(async () => {
		// Check if we have full access to the photos library
		let permission = await ML.getPermissionsAsync();
		// {"accessPrivileges": "none", "canAskAgain": true, "expires": "never", "granted": false, "status": "undetermined"}

		if (
			permission.status === ML.PermissionStatus.UNDETERMINED ||
			(permission.status === ML.PermissionStatus.DENIED && permission.canAskAgain)
		) {
			permission = await ML.requestPermissionsAsync();
		}

		// Permission Denied
		if (permission.status === ML.PermissionStatus.DENIED) {
			Alert.alert(
				'Permission required',
				'You need to grant access to your photos library to import your photos/videos.'
			);
			return;
		}

		// Limited Permission (Can't access path)
		if (permission.accessPrivileges === 'limited') {
			Alert.alert(
				'Limited access',
				'You need to grant full access to your photos library to import your photos/videos.'
			);
			return;
		}

		// If android return error for now...
		if (Platform.OS !== 'ios') {
			Alert.alert('Not supported', 'Not supported for now...');
			return;
		}

		// And for IOS we are assuming every asset is under the same path (which is not the case)

		// file:///Users/xxxx/Library/Developer/CoreSimulator/Devices/F99C471F-C9F9-458D-8B87-BCC4B46C644C/data/Media/DCIM/100APPLE/IMG_0004.JPG
		// file:///var/mobile/Media/DCIM/108APPLE/IMG_8332.JPG‘

		const firstAsset = (await ML.getAssetsAsync({ first: 1 })).assets[0];

		if (!firstAsset) return;

		// Gets asset uri: ph://CC95F08C-88C3-4012-9D6D-64A413D254B3
		const assetId = firstAsset.id;
		// Gets Actual Path
		const path = (await ML.getAssetInfoAsync(assetId)).localUri;

		const libraryPath = Platform.select({
			android: '',
			ios: path.replace('file://', '').split('Media/DCIM/')[0] + 'Media/DCIM/'
		});

		createLocation({
			path: libraryPath,
			indexer_rules_ids: []
		});

		// const assets = await ML.getAssetsAsync({ mediaType: ML.MediaType.photo });
		// assets.assets.map(async (i) => {
		// 	console.log((await ML.getAssetInfoAsync(i)).localUri);
		// });
	}, [createLocation]);

	// const testFN = useCallback(async () => {
	// 	console.log(RFS.PicturesDirectoryPath);

	// 	const firstAsset = (await ML.getAssetsAsync({ first: 1 })).assets[0];
	// 	console.log(firstAsset);
	// 	const assetUri = firstAsset.id;
	// 	const assetDetails = await ML.getAssetInfoAsync(assetUri);
	// 	console.log(assetDetails);
	// 	const path = assetDetails.localUri;
	// 	console.log(path.replace('file://', '').split('Media/DCIM/')[0] + 'Media/DCIM/');
	// 	// const URL = decodeURIComponent(RFS.DocumentDirectoryPath + '/libraries');
	// 	RFS.readdir('/storage/emulated/0/Download/').then((files) => {
	// 		files.forEach((file) => {
	// 			console.log(file);
	// 		});
	// 	});
	// }, []);

	return (
		<Modal ref={modalRef} snapPoints={['20%']}>
			<View style={tw`flex-1 px-6 pt-1 pb-2 bg-app-box`}>
				{/* <Button size="md" variant="accent" style={tw`my-2`} onPress={testFN}>
					<Text>TEST</Text>
				</Button> */}
				<Button size="md" variant="accent" style={tw`my-2`} onPress={handleFilesButton}>
					<Text>Import from Files</Text>
				</Button>
				<Button size="md" variant="accent" onPress={handlePhotosButton}>
					<Text>Import from Photos</Text>
				</Button>
			</View>
		</Modal>
	);
});

export default ImportModal;
