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
			<div className="animation-delay-2 fade-in z-30 flex h-10 flex-row items-center space-x-4">
				{!showWaitlistInput ? (
					<>
						<Button
							onClick={() => setShowWaitlistInput(true)}
							className="z-30 cursor-pointer border-0"
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
							<Github className="mt-[-4px] -ml-1 mr-2 inline h-5 w-5" fill="white" />
							Star on GitHub
						</Button>
					</>
				) : (
					<form onSubmit={handleSubmit(handleWaitlistSubmit)}>
						<div className="flex flex-col justify-center">
							{(waitlistError || waitlistSubmitted) && (
								<div
									className={clsx({
										'bg-opacity/20 my-2 flex flex-row items-center rounded-md border-2 px-2': true,
										'border-red-900 bg-red-800': waitlistError,
										'border-green-900 bg-green-800': !waitlistError,
										'-mt-2': waitlistSubmitted
									})}
								>
									{waitlistError ? (
										<Alert className="mr-1 w-5 fill-red-500" />
									) : (
										<Info className="mr-1 w-5 fill-green-500" />
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
										className={clsx('z-30 cursor-pointer rounded-l-none border-0', {
											'cursor-default opacity-50': loading
										})}
										disabled={loading}
										variant="accent"
										type="submit"
									>
										{loading ? (
											<Spinner className="h-6 w-6 animate-spin fill-white text-white text-opacity-40" />
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
				className={clsx('animation-delay-3 fade-in text-gray-450 z-30 px-6 text-center text-sm', {
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
