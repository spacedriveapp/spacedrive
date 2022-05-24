import { Apple, Github, Linux, Windows } from '@icons-pack/react-simple-icons';
import { Button, Input } from '@sd/ui';
import clsx from 'clsx';
import React, { useEffect } from 'react';
import { useState } from 'react';

export function HomeCTA() {
	const [showWaitlistInput, setShowWaitlistInput] = useState(false);
	const [waitlistEmail, setWaitlistEmail] = useState('');
	return (
		<>
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
					<form
						onSubmit={(e) => {
							e.preventDefault();
							fetch('https://waitlist-api.spacedrive.com/api/expression-of-interest', {
								method: 'POST',
								headers: {
									'Content-Type': 'application/json'
								},
								body: JSON.stringify({
									email: waitlistEmail
								})
							});
						}}
					>
						<div className="flex flex-row">
							<Input
								type="email"
								name="email"
								autoFocus
								value={waitlistEmail}
								onChange={(e) => setWaitlistEmail(e.target.value)}
								placeholder="Enter your email"
								className="rounded-r-none"
							/>
							<Button
								onClick={() => setShowWaitlistInput(true)}
								className="z-30 border-0 rounded-l-none cursor-pointer"
								variant="primary"
								type="submit"
							>
								Submit
							</Button>
						</div>
					</form>
				)}
			</div>
			<p className="z-30 px-6 mt-3 text-sm text-center text-gray-450 animation-delay-3 fade-in">
				{showWaitlistInput ? (
					<>
						We'll keep your place in the queue for early access.
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
