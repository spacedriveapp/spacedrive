import { Card, Divider, Tooltip } from '@sd/ui';
import { motion } from 'framer-motion';
import { useState } from 'react';

import { GoogleLogo, Icon } from '@phosphor-icons/react';
import { Apple, Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import Login from './Login';
import Register from './Register';

const AccountTabs = ['Login', 'Register'] as const;

type SocialLogin = {
	name: "Github" | "Google" | "Apple";
	icon: Icon;
}

const SocialLogins: SocialLogin[] = [
	{name: 'Github', icon: Github},
	{name: 'Google', icon: GoogleLogo},
	{name: 'Apple', icon: Apple},
]

const Tabs = () => {

	const [activeTab, setActiveTab] = useState<'Login' | 'Register'>('Login');

	const socialLoginHandlers = (name: SocialLogin['name']) => {
		return {
			'Github': () => {
				console.log('Github login');
			},
			'Google': () => {
				console.log('Google login');
			},
			'Apple': () => {
				console.log('Apple login');
			}
		}[name]();
	}

	return (
		<Card className="relative flex w-full max-w-[320px] flex-col items-center justify-center !p-0">
			<div className='flex w-full'>
				{AccountTabs.map((text) => (
					<div key={text} onClick={() => {
						setActiveTab(text)
				}} className={clsx("relative flex-1 border-b border-app-line p-2.5 text-center",
						text === 'Login' ? 'rounded-tl-md' : 'rounded-tr-md',
					)}>
						<p className={clsx('relative z-10 text-sm transition-colors duration-200',
							text === activeTab ? 'font-medium text-ink' : 'text-ink-faint'
						)}>{text}</p>
						{text === activeTab && (
						<motion.div
						animate={{
							borderRadius: text === 'Login' ? '0.3rem 0 0 0' : '0 0.3rem 0 0',
						}}
						layoutId='tab' className={clsx("absolute inset-x-0 top-0 z-0 size-full bg-app-line/60"
						)} />
						)}
					</div>
				))}
			</div>
		<div className='flex w-full flex-col justify-center gap-1.5 p-5'>
				{activeTab === 'Login' ? <Login/> : <Register/>}
			<div className='my-2 flex w-full items-center gap-3'>
			<Divider/>
			<p className='text-xs text-ink-faint'>OR</p>
			<Divider/>
			</div>
			<div className='flex justify-center gap-3'>
					{SocialLogins.map((social) => (
						<Tooltip key={social.name} label={social.name} position='bottom'>
						<div onClick={() => socialLoginHandlers(social.name)} key={social.name} className='rounded-full border border-app-line bg-app-input p-3'>
							<social.icon style={{
								fill: 'white'
							}} weight='bold' className='size-4'/>
						</div>
						</Tooltip>
					))}
			</div>
		</div>
		</Card>
	)
}


export default Tabs;
