import { CookieHandlerInterface } from "supertokens-website/utils/cookieHandler/types";
import { nonLibraryClient } from '@sd/client'

let APP_READY = false;

async function getCookiesFromStorage(): Promise<string> {
	if (!APP_READY) {
		return "";
	}
	const cookiesFromStorage = await nonLibraryClient.query(['keys.get'])

	console.log("Cookies from storage (getCookie): ", cookiesFromStorage);

	if (cookiesFromStorage.length === 0) {
		return "";
	}

	/**
	 * Because we store cookies in local storage, we need to manually check
	 * for expiry before returning all cookies
	 */
	const cookieArrayInStorage: string[] = JSON.parse(cookiesFromStorage);
	const cookieArrayToReturn: string[] = [];

	for (let cookieIndex = 0; cookieIndex < cookieArrayInStorage.length; cookieIndex++) {
		const currentCookieString = cookieArrayInStorage[cookieIndex];
		const parts = currentCookieString?.split(";") ?? [];
		let expirationString: string = "";

		for (let partIndex = 0; partIndex < parts.length; partIndex++) {
			const currentPart = parts[partIndex];

			if (currentPart?.toLocaleLowerCase().includes("expires=")) {
				expirationString = currentPart;
				break;
			}
		}

		if (expirationString !== "") {
			const expirationValueString = expirationString.split("=")[1];
			const expirationDate = expirationValueString ? new Date(expirationValueString) : null;
			const currentTimeInMillis = Date.now();

			// if the cookie has expired, we skip it
			if (expirationDate && expirationDate.getTime() < currentTimeInMillis) {
				continue;
			}
		}

		if (currentCookieString !== undefined) {
			cookieArrayToReturn.push(currentCookieString);
		}
	}

	/**
	 * After processing and removing expired cookies we need to update the cookies
	 * in storage so we dont have to process the expired ones again
	 */
	await nonLibraryClient.mutation(['keys.set', JSON.stringify(cookieArrayToReturn)])

	return cookieArrayToReturn.join("; ");
}

async function setCookieToStorage(cookieString: string): Promise<void> {
	if (!APP_READY) {
		return;
	}
	const cookieName = cookieString.split(";")[0]?.split("=")[0];
	console.log("Setting cookie: ", cookieName);

	const cookiesFromStorage = await nonLibraryClient.query(['keys.get'])

	console.log("Cookies from storage: ", cookiesFromStorage);

	let cookiesArray: string[] = [];

	if (cookiesFromStorage.length !== 0) {
		const cookiesArrayFromStorage: string[] = JSON.parse(cookiesFromStorage);
		cookiesArray = cookiesArrayFromStorage;
	}
	console.log("Cookies array: ", cookiesArray);

	let cookieIndex = -1;

	for (let i = 0; i < cookiesArray.length; i++) {
		const currentCookie = cookiesArray[i];

		if (currentCookie?.indexOf(`${cookieName}=`) !== -1) {
			cookieIndex = i;
			break;
		}
	}
	console.log("Cookie index: ", cookieIndex);

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
	console.log("Updated cookies array: ", cookiesArray);

	await nonLibraryClient.mutation(['keys.set', JSON.stringify(cookiesArray)])
}

export default function getCookieHandler(original: CookieHandlerInterface): CookieHandlerInterface {
	return {
		...original,
		getCookie: async function () {
			return getCookiesFromStorage();
		},
		setCookie: async function (cookieString: string) {
			return setCookieToStorage(cookieString);
		},
	};
}

export function setAppReady() {
	APP_READY = true;
}
