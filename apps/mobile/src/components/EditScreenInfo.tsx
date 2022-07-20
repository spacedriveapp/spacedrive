import React from 'react';
import { StyleSheet, Text, TouchableOpacity, View } from 'react-native';

import tw from '../lib/tailwind';

export default function EditScreenInfo({ path }: { path: string }) {
	return (
		<View>
			<View style={styles.getStartedContainer}>
				<Text style={styles.getStartedText}>Open up the code for this screen:</Text>

				<View style={[styles.codeHighlightContainer, styles.homeScreenFilename]}>
					<Text>{path}</Text>
				</View>

				<Text style={tw`bg-red-500`}>
					Change any of the text, save the file, and your app will automatically update.
				</Text>
			</View>

			<View style={styles.helpContainer}>
				<TouchableOpacity style={styles.helpLink}>
					<Text style={styles.helpLinkText}>
						Tap here if your app doesn't automatically update after making changes
					</Text>
				</TouchableOpacity>
			</View>
		</View>
	);
}

const styles = StyleSheet.create({
	getStartedContainer: {
		alignItems: 'center',
		marginHorizontal: 50
	},
	homeScreenFilename: {
		marginVertical: 7
	},
	codeHighlightContainer: {
		borderRadius: 3,
		paddingHorizontal: 4
	},
	getStartedText: {
		fontSize: 17,
		lineHeight: 24,
		textAlign: 'center'
	},
	helpContainer: {
		marginTop: 15,
		marginHorizontal: 20,
		alignItems: 'center'
	},
	helpLink: {
		paddingVertical: 15
	},
	helpLinkText: {
		textAlign: 'center'
	}
});
