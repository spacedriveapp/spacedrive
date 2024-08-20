/* eslint-disable react-hooks/exhaustive-deps */
import { useEffect } from 'react';
import { NavigateFunction, useNavigate } from 'react-router';
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
			toast.error('Oops! Something went wrong.');
		}
	}
	console.log('Navigating to settings');
	navigate('./settings/client/account');
}

export const Component = () => {
	const navigate = useNavigate();

	useEffect(() => {
		handleGoogleCallback(navigate);
	}, []);

	return <></>;
};
