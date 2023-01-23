import { ReactComponent as Alert } from '@sd/assets/svgs/alert.svg';
import { ReactComponent as Info } from '@sd/assets/svgs/info.svg';
import { ReactComponent as Spinner } from '@sd/assets/svgs/spinner.svg';
import { Github } from '@icons-pack/react-simple-icons';
import clsx from 'clsx';
import { useState } from 'react';
import { useForm } from 'react-hook-form';
import { Button, Input } from '@sd/ui';

interface WaitlistInputs {
	email: string;
}

export function HomeCTA() {
	const { register, handleSubmit } = useForm<WaitlistInputs>();

	const [loading, setLoading] = useState(false);
	const [showWaitlistInput, setShowWaitlistInput] = useState(false);
	const [waitlistError, setWaitlistError] = useState('');
	const [waitlistSubmitted, setWaitlistSubmitted] = useState(false);
	const [fire, setFire] = useState<boolean | number>(false);

	const url = import.meta.env.PROD
		? 'https://waitlist-api.spacedrive.com'
		: 'http://localhost:3000';

	async function handleWaitlistSubmit<SubmitHandler>({ email }: WaitlistInputs) {
		if (!email.trim().length) return;

		setLoading(true);

		const req = await fetch(`${url}/api/waitlist`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({
				email
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
			{/* <ReactCanvasConfetti
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
			/> */}
			<div className="z-30 flex flex-row items-center h-10 space-x-4 animation-delay-2 fade-in">
				{!showWaitlistInput ? (
					<>
						<Button
							onClick={() => setShowWaitlistInput(true)}
							className="z-30 border-0 cursor-pointer"
							variant="gray"
						>
							Join Waitlist
						</Button>
						<Button
							href="https://github.com/spacedriveapp/spacedrive"
							target="_blank"
							className="z-30 cursor-pointer"
							variant="accent"
						>
							<Github className="inline w-5 h-5 -mt-[4px] -ml-1 mr-2" fill="white" />
							Star on GitHub
						</Button>
					</>
				) : (
					<form onSubmit={handleSubmit(handleWaitlistSubmit)}>
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
										<Alert className="w-5 mr-1 fill-red-500" />
									) : (
										<Info className="w-5 mr-1 fill-green-500" />
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
									{...register('email')}
									type="email"
									autoFocus
									autoComplete="off"
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
										variant="accent"
										type="submit"
									>
										{loading ? (
											<Spinner className="w-6 h-6 text-white text-opacity-40 animate-spin fill-white" />
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
