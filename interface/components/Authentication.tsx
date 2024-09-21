import { GoogleLogo, Icon } from '@phosphor-icons/react';
import { Apple, Github } from '@sd/assets/svgs/brands';
import { open } from '@tauri-apps/plugin-shell';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { Dispatch, SetStateAction, useState } from 'react';
import { getAuthorisationURLWithQueryParamsAndSetState } from 'supertokens-web-js/recipe/thirdparty';
import { Button, Card, Divider, toast, Tooltip } from '@sd/ui';
import { Icon as Logo } from '~/components';
import { useIsDark } from '~/hooks';

import Login from './Login';
import Register from './Register';

export const AccountTabs = ['Login', 'Register'] as const;

export type SocialLogin = {
	name: 'Github' | 'Google' | 'Apple';
	icon: Icon;
};

export const SocialLogins: SocialLogin[] = [
	{ name: 'Github', icon: Github },
	{ name: 'Google', icon: GoogleLogo },
	{ name: 'Apple', icon: Apple }
];

export const Authentication = ({ reload }: { reload: Dispatch<SetStateAction<boolean>> }) => {
	const [activeTab, setActiveTab] = useState<'Login' | 'Register'>('Login');
	const isDark = useIsDark();

	const socialLoginHandlers = (name: SocialLogin['name']) => {
		return {
			Github: async () => {
				try {
					const authUrl = await getAuthorisationURLWithQueryParamsAndSetState({
						thirdPartyId: 'github',
						frontendRedirectURI: 'http://localhost:9420/api/auth/callback/github'
					});
					await open(authUrl);
				} catch (err: any) {
					if (err.isSuperTokensGeneralError === true) {
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
						frontendRedirectURI: 'spacedrive://-/auth'
					});
					await open(authUrl);
				} catch (err: any) {
					if (err.isSuperTokensGeneralError === true) {
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
						frontendRedirectURI: 'http://localhost:9420/api/auth/callback/apple'
					});
					await open(authUrl);
				} catch (err: any) {
					if (err.isSuperTokensGeneralError === true) {
						toast.error(err.message);
					} else {
						toast.error('Oops! Something went wrong.');
					}
				}
			}
		}[name]();
	};

	return (
		<Card className="bg-app-background relative flex w-full max-w-[400px] flex-col items-center justify-center rounded-lg border border-app-line !p-0 shadow-lg">
			<div className="flex w-full">
				{AccountTabs.map((text) => (
					<div
						key={text}
						onClick={() => setActiveTab(text)}
						className={clsx(
							'relative flex-1 cursor-pointer border-b border-app-line p-3 text-center transition-colors duration-200',
							text === 'Login' ? 'rounded-tl-lg' : 'rounded-tr-lg',
							text === activeTab ? 'bg-app-background-alt' : ''
						)}
					>
						<p
							className={clsx(
								'relative z-10 text-sm transition-colors duration-200',
								text === activeTab ? 'font-semibold text-ink' : 'text-ink-faint'
							)}
						>
							{text}
						</p>
						{text === activeTab && (
							<motion.div
								animate={{
									borderRadius: text === 'Login' ? '0.5rem 0 0 0' : '0 0.5rem 0 0'
								}}
								layoutId="tab"
								className="absolute inset-x-0 top-0 z-0 h-full w-full bg-app-line/60"
							/>
						)}
					</div>
				))}
			</div>
			<div className="flex w-full flex-col items-center gap-4 p-6">
				<div className="flex items-center justify-center gap-2">
					<Logo size={36} name="Ball" />
					<h3
						className={clsx(
							'text-xl font-extrabold',
							isDark ? 'text-white' : 'text-black'
						)}
					>
						Spacedrive
					</h3>
				</div>
				{activeTab === 'Login' ? <Login reload={reload} /> : <Register />}
				{/* Optionally, uncomment the social login block when ready */}
				{/* <div className="my-4 flex w-full items-center gap-3">
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
                                className="rounded-full border border-app-line bg-app-input p-3"
                            >
                                <social.icon
                                    style={{ fill: 'white' }}
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
