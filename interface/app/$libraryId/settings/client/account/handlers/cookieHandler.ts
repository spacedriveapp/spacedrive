import { CookieHandlerInterface } from 'supertokens-website/utils/cookieHandler/types';

const frontendCookiesKey = 'frontendCookies';

/**
 * Tauri handles cookies differently than in browser environments. The SuperTokens
 * SDK uses frontend cookies, to make sure everything work correctly we add custom
 * cookie handling and store cookies in local storage instead (This is not a problem
 * since these are frontend cookies and not server side cookies)
 */
function getCookiesFromStorage(): string {
	const cookiesFromStorage = window.localStorage.getItem(frontendCookiesKey);

	if (cookiesFromStorage === null) {
		window.localStorage.setItem(frontendCookiesKey, '[]');
		return '';
	}

	/**
	 * Because we store cookies in local storage, we need to manually check
	 * for expiry before returning all cookies
	 */
	const cookieArrayInStorage: string[] = JSON.parse(cookiesFromStorage);
	const cookieArrayToReturn: string[] = [];

	for (let cookieIndex = 0; cookieIndex < cookieArrayInStorage.length; cookieIndex++) {
		const currentCookieString = cookieArrayInStorage[cookieIndex];
		const parts = currentCookieString?.split(';');
		let expirationString: string = '';

		for (let partIndex = 0; partIndex < parts!.length; partIndex++) {
			const currentPart = parts![partIndex];

			if (currentPart!.toLocaleLowerCase().includes('expires=')) {
				expirationString = currentPart!;
				break;
			}
		}

		if (expirationString !== '') {
			const expirationValueString = expirationString.split('=')[1];
			const expirationDate = new Date(expirationValueString!);
			const currentTimeInMillis = Date.now();

			// if the cookie has expired, we skip it
			if (expirationDate.getTime() < currentTimeInMillis) {
				continue;
			}
		}

		cookieArrayToReturn.push(currentCookieString!);
	}

	/**
	 * After processing and removing expired cookies we need to update the cookies
	 * in storage so we dont have to process the expired ones again
	 */
	window.localStorage.setItem(frontendCookiesKey, JSON.stringify(cookieArrayToReturn));

	return cookieArrayToReturn.join('; ');
}

function setCookieToStorage(cookieString: string) {
	const cookieName = cookieString.split(';')[0]!.split('=')[0];
	const cookiesFromStorage = window.localStorage.getItem(frontendCookiesKey);
	let cookiesArray: string[] = [];

	if (cookiesFromStorage !== null) {
		const cookiesArrayFromStorage: string[] = JSON.parse(cookiesFromStorage);
		cookiesArray = cookiesArrayFromStorage;
	}

	let cookieIndex = -1;

	for (let i = 0; i < cookiesArray.length; i++) {
		const currentCookie = cookiesArray[i];

		if (currentCookie!.indexOf(`${cookieName}=`) !== -1) {
			cookieIndex = i;
			break;
		}
	}

	/**
	 * If a cookie with the same name already exists (index != -1) then we
	 * need to remove the old value and replace it with the new one.
	 *
	 * If it does not exist then simply add the new cookie
	 */
	if (cookieIndex !== -1) {
		cookiesArray[cookieIndex] = cookieString;
	} else {
		cookiesArray.push(cookieString);
	}

	window.localStorage.setItem(frontendCookiesKey, JSON.stringify(cookiesArray));
}

export default function getCookieHandler(original: CookieHandlerInterface): CookieHandlerInterface {
	return {
		...original,
		getCookie: async function () {
			const cookies = getCookiesFromStorage();
			return cookies;
		},
		setCookie: async function (cookieString: string) {
			setCookieToStorage(cookieString);
		}
	};
}
