import { ReactComponent as Alert } from '@sd/assets/svgs/alert.svg';
import { Github } from '@sd/assets/svgs/brands';
import { ReactComponent as Info } from '@sd/assets/svgs/info.svg';
import { ReactComponent as Spinner } from '@sd/assets/svgs/spinner.svg';
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

	async function handleWaitlistSubmit({ email }: WaitlistInputs) {
		if (!email.trim().length) return;

		setLoading(true);

		try {
			const req = await fetch(`https://app.spacedrive.com/api/v1/waitlist`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify({ email })
			});

			if (!req.ok) {
				return setWaitlistError('An error occurred. Please try again.');
			}

			const response = (await req.json()) as { success: boolean; message: string };

			if (!response.success) {
				return setWaitlistError(response.message);
			}
			setWaitlistSubmitted(true);
		} catch (e: any) {
			throw new Error(e.message);
		} finally {
			setLoading(false);
		}
		// Remove error after a few seconds
		setTimeout(() => {
			setWaitlistError('');
		}, 5000);
	}

	return (
		<>
			<div className="animation-delay-2 z-30 flex h-10 flex-row items-center space-x-4 fade-in">
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
							<Github className="-ml-1 mr-2 mt-[-4px] inline h-5 w-5" fill="white" />
							Star on GitHub
						</Button>
					</>
				) : (
					<form onSubmit={handleSubmit(handleWaitlistSubmit)}>
						<div className="flex flex-col justify-center">
							{(waitlistError || waitlistSubmitted) && (
								<div
									className={clsx({
										'my-2 flex flex-row items-center rounded-md border-2 px-2':
											true,
										'border-red-900 bg-red-800/20': waitlistError,
										'border-green-900 bg-green-800/20': !waitlistError,
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
							<div className="flex flex-row">
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
									size="lg"
									disabled={waitlistSubmitted}
								/>
								{!waitlistSubmitted && (
									<Button
										onClick={() => setShowWaitlistInput(true)}
										className={clsx(
											'z-30 cursor-pointer rounded-l-none border-0',
											{
												'cursor-default opacity-50': loading
											}
										)}
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
				className={clsx(
					'animation-delay-3 z-30 px-6 text-center text-sm text-gray-400 fade-in',
					{
						'mt-10': waitlistError,
						'mt-3': !waitlistError
					}
				)}
			>
				{showWaitlistInput ? (
					<>
						We&apos;ll keep your place in the queue for early access.
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
