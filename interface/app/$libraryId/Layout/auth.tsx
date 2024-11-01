/* eslint-disable no-restricted-syntax */
/* eslint-disable react-hooks/exhaustive-deps */
import { useEffect } from 'react';
import { NavigateFunction, useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { clearLoginAttemptInfo, consumeCode } from 'supertokens-web-js/recipe/passwordless';
import { signInAndUp } from 'supertokens-web-js/recipe/thirdparty';
import { toast } from '@sd/ui';

async function handleGoogleCallback(navigate: NavigateFunction) {
	try {
		const response = await signInAndUp();

		if (response.status === 'OK') {
			console.log(response.user);
			if (response.createdNewRecipeUser && response.user.loginMethods.length === 1) {
				toast.success('Account created successfully');
			} else {
				toast.success('Signed in successfully');
			}
		} else if (response.status === 'SIGN_IN_UP_NOT_ALLOWED') {
			// the reason string is a user friendly message
			// about what went wrong. It can also contain a support code which users
			// can tell you so you know why their sign in / up was not allowed.
			toast.error(response.reason);
		} else {
			// SuperTokens requires that the third party provider
			// gives an email for the user. If that's not the case, sign up / in
			// will fail.

			// As a hack to solve this, you can override the backend functions to create a fake email for the user.

			toast.error('No email provided by social login. Please use another form of login');
		}
	} catch (err: any) {
		if (err.isSuperTokensGeneralError === true) {
			// this may be a custom error message sent from the API by you.
			toast.error(err.message);
		} else {
			console.error(err);
			toast.error('Oops! Something went wrong.');
		}
	}
	console.log('Navigating to settings');
	navigate(-1);
}

async function handleMagicLinkClicked(navigate: NavigateFunction) {
	try {
		const response = await consumeCode();
		console.log('consumeCode response', response);

		if (response.status === 'OK') {
			// we clear the login attempt info that was added when the createCode function
			// was called since the login was successful.
			await clearLoginAttemptInfo();
			if (response.createdNewRecipeUser && response.user.loginMethods.length === 1) {
				// user sign up success
			} else {
				// user sign in success
			}
		} else {
			// this can happen if the magic link has expired or is invalid
			// or if it was denied due to security reasons in case of automatic account linking

			// we clear the login attempt info that was added when the createCode function
			// was called - so that if the user does a page reload, they will now see the
			// enter email / phone UI again.
			await clearLoginAttemptInfo();
			toast.error('Login failed. Please try again');
			navigate(-1);
		}
	} catch (err: any) {
		if (err.isSuperTokensGeneralError === true) {
			// this may be a custom error message sent from the API by you.
			toast.error(err.message);
		} else {
			console.error(err);
			toast.error('Oops! Something went wrong.');
		}
	}
	console.log('Navigating to settings');
	navigate(-1);
}

export const Component = () => {
	const navigate = useNavigate();
	const [query] = useSearchParams();
	const { hash } = useLocation();

	useEffect(() => {
		(window.location as any).__TEMP_URL_PARAMS = query;
		(window.location as any).__TEMP_URL_HASH = hash;
		handleMagicLinkClicked(navigate);
	}, []);

	return <></>;
};
