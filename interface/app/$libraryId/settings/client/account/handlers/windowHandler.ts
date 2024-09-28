import { WindowHandlerInterface } from 'supertokens-website/utils/windowHandler/types';

/**
 * This example app uses HashRouter from react-router-dom. The SuperTokens SDK relies on
 * some window properties like location hash, query params etc. Because HashRouter places
 * everything other than the website base in the location hash, we need to add custom
 * handling for some of the properties of the Window API
 */
export default function getWindowHandler(original: WindowHandlerInterface): WindowHandlerInterface {
	return {
		...original,
		location: {
			...original.location,
			getSearch: function () {
				const params: URLSearchParams | string =
					(window.location as any).__TEMP_URL_PARAMS ?? '';
				console.log('params', params);
				return params.toString();
			},
			getHash: function () {
				// Location hash always starts with a #, when returning we prepend it
				const locationHash: string = (window.location as any).__TEMP_URL_HASH ?? '';
				console.log('locationHash', locationHash);
				return locationHash;
			},
			getOrigin: function () {
				return 'http://localhost:8001';
			},
			getHostName: function () {
				return 'localhost';
			},
			getPathName: function () {
				let locationHash = window.location.hash;

				if (locationHash === '') {
					return '';
				}

				if (locationHash.startsWith('#')) {
					// Remove the starting pound symbol
					locationHash = locationHash.substring(1);
				}

				locationHash = locationHash.split('?')[0] ?? '';

				if (locationHash.includes('#')) {
					// Remove location hash
					locationHash = locationHash.split('#')[0] ?? '';
				}

				return locationHash;
			}
		}
	};
}
