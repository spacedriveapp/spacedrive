import { Github } from '@icons-pack/react-simple-icons';
import { Button, Input } from '@sd/ui';
import clsx from 'clsx';
import React, { FormEvent, useState } from 'react';
import ReactCanvasConfetti from 'react-canvas-confetti';

import { Alert } from '../../../../packages/interface/src/components/icons/Alert';
import { Info } from '../../../../packages/interface/src/components/icons/Info';

export function HomeCTA() {
	const [showWaitlistInput, setShowWaitlistInput] = useState(false);
	const [loading, setLoading] = useState(false);
	const [waitlistError, setWaitlistError] = useState('');
	const [waitlistSubmitted, setWaitlistSubmitted] = useState(false);
	const [waitlistEmail, setWaitlistEmail] = useState('');
	const [fire, setFire] = useState<boolean | number>(false);

	async function handleWaitlistSubmit(e: FormEvent<HTMLFormElement>) {
		e.preventDefault();
		if (!waitlistEmail.trim().length) return;

		setLoading(true);

		const req = await fetch('https://waitlist-api.spacedrive.com/api/waitlist', {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				email: waitlistEmail
			})
		});

		if (req.status === 200) {
			setWaitlistError('');
			setFire(Math.random());
			setWaitlistSubmitted(true);
			setLoading(false);
		} else if (req.status >= 400 && req.status < 500) {
			const res = await req.json();
			setWaitlistError(res.message);

			// Remove error after a few seconds
			setTimeout(() => {
				setWaitlistError('');
			}, 5000);
		}

		setLoading(false);
	}

	return (
		<>
			<ReactCanvasConfetti
				fire={fire}
				angle={44}
				className="absolute top-48"
				colors={['#26ccff', '#a25afd']}
				decay={0.8}
				drift={1}
				gravity={1}
				origin={{
					x: 0.5,
					y: 0.5
				}}
				particleCount={55}
				resize
				scalar={1}
				shapes={['circle', 'square']}
				spread={360}
				startVelocity={45}
				ticks={600}
				useWorker
				zIndex={-1}
			/>
			<div className="z-30 flex flex-row items-center h-10 space-x-4 animation-delay-2 fade-in">
				{!showWaitlistInput ? (
					<>
						<Button
							onClick={() => setShowWaitlistInput(true)}
							className="z-30 border-0 cursor-pointer"
							variant="primary"
						>
							Join Waitlist
						</Button>
						<Button
							href="https://github.com/spacedriveapp/spacedrive"
							target="_blank"
							className="z-30 cursor-pointer"
							variant="gray"
						>
							<Github className="inline w-5 h-5 -mt-[4px] -ml-1 mr-2" fill="white" />
							Star on GitHub
						</Button>
					</>
				) : (
					<form onSubmit={handleWaitlistSubmit}>
						<div className="flex flex-col justify-center">
							{(waitlistError || waitlistSubmitted) && (
								<div
									className={clsx({
										'flex flex-row items-center bg-opacity-20 border-2 my-2 px-2 rounded-md': true,
										'bg-red-800 border-red-900': waitlistError,
										'bg-green-800 border-green-900': !waitlistError,
										'-mt-2': waitlistSubmitted
									})}
								>
									{waitlistError ? (
										<Alert className="fill-red-500 w-5 mr-1" />
									) : (
										<Info className="fill-green-500 w-5 mr-1" />
									)}
									<p
										className={clsx({
											'text-sm': true,
											'text-red-500': waitlistError,
											'text-green-500': !waitlistError
										})}
									>
										{waitlistError || 'You have been added to the waitlist'}
									</p>
								</div>
							)}
							<div className={'flex flex-row'}>
								<Input
									type="email"
									name="email"
									autoFocus
									value={waitlistEmail}
									autoComplete="off"
									onChange={(e) => setWaitlistEmail(e.target.value)}
									placeholder="Enter your email"
									className={clsx({
										'hidden': waitlistSubmitted,
										'rounded-r-none': !waitlistSubmitted
									})}
									disabled={waitlistSubmitted}
								/>
								{!waitlistSubmitted && (
									<Button
										onClick={() => setShowWaitlistInput(true)}
										className={clsx('z-30 border-0 rounded-l-none cursor-pointer', {
											'opacity-50 cursor-default': loading
										})}
										disabled={loading}
										variant="primary"
										type="submit"
									>
										{loading ? (
											<svg
												role="status"
												className="w-6 h-6 text-white text-opacity-40 animate-spin fill-white"
												viewBox="0 0 100 101"
												fill="none"
												xmlns="http://www.w3.org/2000/svg"
											>
												<path
													d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z"
													fill="currentColor"
												></path>
												<path
													d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z"
													fill="currentFill"
												></path>
											</svg>
										) : (
											'Submit'
										)}
									</Button>
								)}
							</div>
						</div>
					</form>
				)}
			</div>
			<p
				className={clsx('z-30 px-6 text-sm text-center text-gray-450 animation-delay-3 fade-in', {
					'mt-10': waitlistError,
					'mt-3': !waitlistError
				})}
			>
				{showWaitlistInput ? (
					<>
						We'll keep your place in the queue for early access.
						<br />
						<br />
					</>
				) : waitlistSubmitted ? (
					<>
						You have been added to the waitlist.
						<br />
						<br />
					</>
				) : (
					<>
						Coming soon on macOS, Windows and Linux.
						<br />
						Shortly after to iOS & Android.
					</>
				)}
			</p>
		</>
	);
}

export default HomeCTA;
