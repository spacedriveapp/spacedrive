import { GoogleLogo, Icon } from '@phosphor-icons/react';
import { Apple, Github } from '@sd/assets/svgs/brands';
import { RSPCError } from '@spacedrive/rspc-client';
import { UseMutationResult } from '@tanstack/react-query';
import { open } from '@tauri-apps/plugin-shell';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { Dispatch, SetStateAction, useState } from 'react';
import { getAuthorisationURLWithQueryParamsAndSetState } from 'supertokens-web-js/recipe/thirdparty';
import { Card, toast } from '@sd/ui';
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

export const Authentication = ({
	reload,
	cloudBootstrap
}: {
	reload: Dispatch<SetStateAction<boolean>>;
	cloudBootstrap: UseMutationResult<null, RSPCError, [string, string], unknown>; // Cloud bootstrap mutation
}) => {
	const [activeTab, setActiveTab] = useState<'Login' | 'Register'>('Login');
	const isDark = useIsDark();

	// Currently not in use due to backend issues - @Rocky43007
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
		<Card className="bg-app-background relative flex w-full flex-col items-center justify-center rounded-lg border border-app-line !p-0 shadow-lg">
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
									borderRadius: text === 'Login' ? '0.3rem 0 0 0' : '0 0.3rem 0 0'
								}}
								layoutId="tab"
								className="absolute inset-x-0 top-0 z-0 size-full bg-app-line/60"
							/>
						)}
					</div>
				))}
			</div>
			<div className="flex w-full flex-col items-center gap-4 p-6">
				<div className="flex items-center justify-center gap-1">
					<Logo size={36} name="Ball" />
					<h3
						className={clsx(
							'text-xl font-semibold',
							isDark ? 'text-white' : 'text-black'
						)}
					>
						Spacedrive Cloud
					</h3>
				</div>
				{activeTab === 'Login' ? (
					<Login reload={reload} cloudBootstrap={cloudBootstrap} />
				) : (
					<Register />
				)}
				<div className="text-center text-sm text-ink-faint">
					Social auth and SSO (Single Sign On) available soon!
				</div>
				{/* Optionally, uncomment the social login block when ready */}
				{/* <div className="flex items-center w-full gap-3 my-4">
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
                                className="p-3 border rounded-full border-app-line bg-app-input"
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
