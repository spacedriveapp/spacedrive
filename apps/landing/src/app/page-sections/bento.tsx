import { ArrowRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import Image from 'next/image';

import { CtaButton } from '../../components/cta-button';
import { Icon, IconName } from '../Icon';

interface BentoCardProps {
	icon?: IconName;
	title: string;
	size?: 'small' | 'large';
	children?: React.ReactNode;
	className?: string;
	description?: string;
}

const BENTO_BASE_CLASS = `bento-border-left relative flex flex-col rounded-[10px] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] overflow-hidden group`;
const BENTO_TITLE_CLASS = 'text-md font-bold text-ink-dull';

function BentoCard({
	icon,
	title,
	size = 'small',
	children,
	className,
	description
}: BentoCardProps) {
	return (
		<div
			className={clsx(
				BENTO_BASE_CLASS,
				size === 'small' ? 'h-[200px] w-full' : 'col-span-2 row-span-2 h-[420px] w-full',
				'items-center justify-start p-5',
				className
			)}
		>
			<div className="flex w-full flex-col gap-3">
				{icon && <Icon name={icon} size={80} />}
				<h3 className={BENTO_TITLE_CLASS}>{title}</h3>
			</div>
			{children}
			{description && (
				<div className="">
					<p className="text-left text-sm text-ink-faint">{description}</p>
				</div>
			)}
		</div>
	);
}

export function Bento() {
	return (
		<div className="container mx-auto p-4">
			<div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4">
				<BentoCard
					icon="Spacedrop1"
					title="Spacedrop"
					description="Securely share files with anyone, anywhere using our encrypted file sharing system."
				/>
				<BentoCard
					icon="Sync"
					title="P2P Sync"
					description="Synchronize your files across devices with peer-to-peer technology."
				/>
				<BentoCard
					icon="Package"
					title="Archival Tools"
					description="Preserve and organize your digital archives with powerful compression tools."
				/>
				<BentoCard
					icon="Database"
					title="Local Database"
					description="Fast, reliable local-first database for your file management needs."
				/>
				<BentoCard title="" size="large" className="flex-col items-center justify-center">
					<Image
						quality={100}
						src="/images/cloud.png"
						width={380}
						height={380}
						alt="Spacedrive vault"
					/>
					<div className="mb-8 text-center">
						<h3 className="text-2xl font-bold">Spacedrive Cloud</h3>
						<p className="text-sm text-ink-faint">
							Store your files in the cloud with our secure cloud storage solution.
						</p>
						<CtaButton className="my-4" href="/pricing" highlighted>
							View Plans
						</CtaButton>
					</div>
				</BentoCard>
				<BentoCard
					icon="Lock"
					title="File Encryption"
					description="Keep your files secure with end-to-end encryption."
				/>
				<BentoCard
					icon="CollectionSparkle"
					title="AI Labeling"
					description="Automatically organize your files with AI-powered tagging and categorization."
				/>
				<BentoCard
					icon="Video"
					title="Video Encoding"
					description="Convert and optimize your videos with our built-in encoding tools."
				/>
				<BentoCard
					icon="Movie"
					title="In-app Player"
					description="Play your media files directly within Spacedrive with our feature-rich player."
				/>
			</div>
		</div>
	);
}
