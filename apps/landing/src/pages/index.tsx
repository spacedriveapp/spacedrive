import { Apple, Github, Linux, Windows } from '@icons-pack/react-simple-icons';
import { Button } from '@sd/ui';
import clsx from 'clsx';
import React, { useEffect } from 'react';
import { useState } from 'react';

import AppEmbed from '../components/AppEmbed';
import { Bubbles } from '../components/Bubbles';
import { Footer } from '../components/Footer';
import NavBar from '../components/NavBar';

interface SectionProps {
	orientation: 'left' | 'right';
	heading?: string;
	description?: string | React.ReactNode;
	children?: React.ReactNode;
}

function Section(props: SectionProps = { orientation: 'left' }) {
	let info = (
		<div className="p-10">
			{props.heading && <h1 className="text-4xl font-black">{props.heading}</h1>}
			{props.description && <p className="mt-5 text-xl text-gray-450">{props.description}</p>}
		</div>
	);
	return (
		<div className="grid grid-cols-1 my-10 lg:grid-cols-2 lg:my-44">
			{props.orientation === 'right' ? (
				<>
					{info}
					{props.children}
				</>
			) : (
				<>
					{props.children}
					{info}
				</>
			)}
		</div>
	);
}

function Page() {
	// const [appLoaded, setAppLoaded] = useState(false);

	// function handleResize(event: Event) {
	//   if (window.innerWidth > 1000) setShowApp(true);
	//   else if (showApp) setShowApp(false);
	// }

	// useEffect(() => {
	//   window.addEventListener('resize', handleResize);
	//   return () => window.removeEventListener('resize', handleResize);
	// }, []);

	return (
		<>
			<div className="mt-28 lg:mt-36" />
			<h1
				id="content"
				className="px-2 mb-3 text-4xl font-black leading-tight text-center md:text-6xl "
			>
				A file explorer from the future.
			</h1>
			<p className="max-w-4xl mt-1 mb-8 text-center text-md lg:text-lg leading-2 lg:leading-8 text-gray-450">
				Combine your drives and clouds into one database that you can organize and explore from any
				device.
				<br />
				<span className="hidden sm:block">
					Designed for creators, hoarders and the painfully disorganized.
				</span>
			</p>
			<div className="flex flex-row space-x-4">
				{/* <Button className="px-2">
          <WindowsLogo className="" fill="white" />
        </Button> */}

				{/* <Button
          onClick={() =>
            alert(
              "You're here early! This is the only button on this page that does not work, I promise. Release build coming very soon—follow @spacedriveapp for updates."
            )
          }
          className="opacity-50 cursor-not-allowed select-none"
          variant="primary"
        >
          Download
        </Button> */}

				<Button
					href="https://github.com/spacedriveapp/spacedrive"
					target="_blank"
					className="cursor-pointer"
					variant="gray"
				>
					<Github className="inline w-5 h-5 -mt-[4px] -ml-1 mr-2" fill="white" />
					Star on GitHub
				</Button>
			</div>
			<p className="px-6 mt-3 text-sm text-center opacity-75 text-gray-450">
				Coming soon on macOS, Windows and Linux.
				<br />
				Shortly after to iOS & Android.
			</p>

			<AppEmbed />
			<Section
				orientation="right"
				heading="Never leave a file behind."
				description={
					<>
						Spacedrive accounts for every file you own, uniquely fingerprinting and extracting
						metadata so you can sort, tag, backup and share files without limitations of any one
						cloud provider.
						<br />
						<br />
						<a
							className="transition text-primary-600 hover:text-primary-500"
							href="https://github.com/spacedriveapp"
							target="_blank"
						>
							Find out more →
						</a>
					</>
				}
			/>
			<Bubbles />
		</>
	);
}

export default Page;
