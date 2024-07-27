import { CookieHandlerInterface } from "supertokens-website/utils/cookieHandler/types";
import { nonLibraryClient } from '@sd/client'

function getCookiesFromStorage(): string {
	let cookiesFromStorage: string = "";

	nonLibraryClient.query(['keys.get']).then((response) => {
		// Debugging
		console.log("rspc response: ", response);
		const cookiesArrayFromStorage: string[] = JSON.parse(response);
		console.log("Cookies fetched from storage: ", cookiesArrayFromStorage);

		// Actual
		cookiesFromStorage = response;
	}).catch((e) => {
		console.error("Error fetching cookies from storage: ", e);
	});

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
	// window.localStorage.setItem(frontendCookiesKey, JSON.stringify(cookieArrayToReturn));
	nonLibraryClient.mutation(['keys.set', JSON.stringify(cookieArrayToReturn)]).then(() => {
		console.log("Cookies set successfully");
	}).catch((e) => {
		console.error("Error setting cookies to storage: ", e);
		return;
	})

	return cookieArrayToReturn.join("; ");
}

function setCookieToStorage(cookieString: string) {
	const cookieName = cookieString.split(";")[0]?.split("=")[0];

	let cookiesFromStorage: string = "";
	nonLibraryClient.query(['keys.get']).then((response) => {
		// Debugging
		const cookiesArrayFromStorage: string[] = JSON.parse(response);
		console.log("Cookies fetched from storage: ", cookiesArrayFromStorage);

		// Actual
		cookiesFromStorage = response;
	}).catch((e) => {
		console.error("Error fetching cookies from storage: ", e);
	});

	let cookiesArray: string[] = [];

	if (cookiesFromStorage.length !== 0) {
		const cookiesArrayFromStorage: string[] = JSON.parse(cookiesFromStorage);
		cookiesArray = cookiesArrayFromStorage;
	}

	let cookieIndex = -1;

	for (let i = 0; i < cookiesArray.length; i++) {
		const currentCookie = cookiesArray[i];

		if (currentCookie?.indexOf(`${cookieName}=`) !== -1) {
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

	nonLibraryClient.mutation(['keys.set', JSON.stringify(cookiesArray)]).then(() => {
		console.log("Cookies set successfully");
	}).catch((e) => {
		console.error("Error setting cookies to storage: ", e);
		return;
	})

	console.log("Setting cookies to storage: ", cookiesArray);
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
		},
	};
}
