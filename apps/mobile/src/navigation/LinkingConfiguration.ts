/**
 * Learn more about deep linking with React Navigation
 * https://reactnavigation.org/docs/deep-linking
 * https://reactnavigation.org/docs/configuring-links
 */
import { LinkingOptions } from '@react-navigation/native';
import * as Linking from 'expo-linking';

import { RootStackParamList } from '.';

// TODO: Deep linking for React Navigation. It will allow us to do spacedrive://tags/{id} etc.
const linking: LinkingOptions<RootStackParamList> = {
	prefixes: [Linking.createURL('/')],
	config: {
		screens: {
			Root: {
				screens: {
					Home: 'home'
				}
			},
			Settings: 'settings',
			NotFound: '*'
		}
	}
};

export default linking;
