import { GoogleLogo, Icon } from '@phosphor-icons/react';
import { Apple, Github } from '@sd/assets/svgs/brands';
import { open } from '@tauri-apps/plugin-shell';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { useState } from 'react';
import { getAuthorisationURLWithQueryParamsAndSetState } from 'supertokens-web-js/recipe/thirdparty';
import { Button, Card, Divider, toast, Tooltip } from '@sd/ui';

import Login from './Login';
import Register from './Register';

const AccountTabs = ['Login', 'Register'] as const;

type SocialLogin = {
	name: 'Github' | 'Google' | 'Apple';
	icon: Icon;
};

const SocialLogins: SocialLogin[] = [
	{ name: 'Github', icon: Github },
	{ name: 'Google', icon: GoogleLogo },
	{ name: 'Apple', icon: Apple }
];

const Tabs = () => {
	const [activeTab, setActiveTab] = useState<'Login' | 'Register'>('Login');

	// Currently opens in App.
	const socialLoginHandlers = (name: SocialLogin['name']) => {
		return {
			Github: async () => {
				try {
					const authUrl = await getAuthorisationURLWithQueryParamsAndSetState({
						thirdPartyId: 'github',

						// This is where Github should redirect the user back after login or error.
						frontendRedirectURI: 'http://localhost:9420/api/auth/callback/github'
					});

					// we redirect the user to Github for auth.
					await open(authUrl);
				} catch (err: any) {
					if (err.isSuperTokensGeneralError === true) {
						// this may be a custom error message sent from the API by you.
						toast.error(err.message);
					} else {
						toast.error('Oops! Something went wrong.');
					}
				}
			},
			Google: async () => {
				try {
					const authUrl = await getAuthorisationURLWithQueryParamsAndSetState({
						thirdPartyId: 'google',

						// This is where Google should redirect the user back after login or error.
						// This URL goes on the Google's dashboard as well.
						frontendRedirectURI: 'spacedrive://-/auth'
					});

					/*
					Example value of authUrl: https://accounts.google.com/o/oauth2/v2/auth/oauthchooseaccount?scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fuserinfo.email&access_type=offline&include_granted_scopes=true&response_type=code&client_id=1060725074195-kmeum4crr01uirfl2op9kd5acmi9jutn.apps.googleusercontent.com&state=5a489996a28cafc83ddff&redirect_uri=https%3A%2F%2Fsupertokens.io%2Fdev%2Foauth%2Fredirect-to-app&flowName=GeneralOAuthFlow
					*/

					// we redirect the user to google for auth.
					await open(authUrl);
				} catch (err: any) {
					if (err.isSuperTokensGeneralError === true) {
						// this may be a custom error message sent from the API by you.
						toast.error(err.message);
					} else {
						toast.error('Oops! Something went wrong.');
					}
				}
			},
			Apple: async () => {
				try {
					const authUrl = await getAuthorisationURLWithQueryParamsAndSetState({
						thirdPartyId: 'apple',

						// This is where Apple should redirect the user back after login or error.
						frontendRedirectURI: 'http://localhost:9420/api/auth/callback/apple'
					});

					// we redirect the user to Apple for auth.
					await open(authUrl);
				} catch (err: any) {
					if (err.isSuperTokensGeneralError === true) {
						// this may be a custom error message sent from the API by you.
						toast.error(err.message);
					} else {
						toast.error('Oops! Something went wrong.');
					}
				}
			}
		}[name]();
	};

	return (
		<Card className="relative flex w-full max-w-[320px] flex-col items-center justify-center !p-0">
			<div className="flex w-full">
				{AccountTabs.map((text) => (
					<div
						key={text}
						onClick={() => {
							setActiveTab(text);
						}}
						className={clsx(
							'relative flex-1 border-b border-app-line p-2.5 text-center',
							text === 'Login' ? 'rounded-tl-md' : 'rounded-tr-md'
						)}
					>
						<p
							className={clsx(
								'relative z-10 text-sm transition-colors duration-200',
								text === activeTab ? 'font-medium text-ink' : 'text-ink-faint'
							)}
						>
							{text}
						</p>
						{text === activeTab && (
							<motion.div
								animate={{
									borderRadius: text === 'Login' ? '0.3rem 0 0 0' : '0 0.3rem 0 0'
								}}
								layoutId="tab"
								className={clsx(
									'absolute inset-x-0 top-0 z-0 size-full bg-app-line/60'
								)}
							/>
						)}
					</div>
				))}
			</div>
			<div className="flex w-full flex-col justify-center gap-1.5 p-5">
				{activeTab === 'Login' ? <Login /> : <Register />}
				{/* Disabling for now for demo purposes. We need to figure out on the backend how the tokens are recieved so we can a) store them in the frontend and b) use them as auth tokens for our cloud services. - @Rocky43007 */}
				{/* <div className="my-2 flex w-full items-center gap-3">
					<Divider />
					<p className="text-xs text-ink-faint">OR</p>
					<Divider />
				</div>
				<div className="flex justify-center gap-3">
					{SocialLogins.map((social) => (
						<Tooltip key={social.name} label={social.name} position="bottom">
							<Button
								variant="outline"
								onClick={async () => await socialLoginHandlers(social.name)}
								key={social.name}
								className="rounded-full border border-app-line bg-app-input p-3"
							>
								<social.icon
									style={{
										fill: 'white'
									}}
									weight="bold"
									className="size-4"
								/>
							</Button>
						</Tooltip>
					))}
				</div> */}
			</div>
		</Card>
	);
};

export default Tabs;
