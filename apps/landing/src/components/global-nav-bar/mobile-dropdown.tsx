'use client';

import { Book, Chat, DotsThreeVertical, MapPin, User } from '@phosphor-icons/react';
import { Academia, Discord, Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import { AppRouterInstance } from 'next/dist/shared/lib/app-router-context.shared-runtime';
import { usePathname, useRouter } from 'next/navigation';
import { Button, Dropdown } from '@sd/ui';

import { positions } from '../../app/careers/data';

export function MobileDropdown() {
	const router = useRouter();
	const pathname = usePathname();

	const link = (path: string) => rawLink(path, router, pathname);

	return (
		<Dropdown.Root
			button={
				<Button aria-label="mobile-menu" className="hover:!bg-transparent" size="icon">
					<DotsThreeVertical weight="bold" className="size-6" />
				</Button>
			}
			className="right-4 top-2 block text-white lg:hidden"
			itemsClassName="!rounded-2xl shadow-2xl shadow-black p-2 !bg-gray-850 mt-2 !border-gray-500 text-[15px]"
		>
			<Dropdown.Section>
				<a href="https://discord.gg/gTaF2Z44f5" target="_blank">
					<Dropdown.Item icon={Discord}>Join Discord</Dropdown.Item>
				</a>
				<a href="https://github.com/spacedriveapp/spacedrive" target="_blank">
					<Dropdown.Item icon={Github}>Repository</Dropdown.Item>
				</a>
			</Dropdown.Section>
			<Dropdown.Section>
				<Dropdown.Item icon={MapPin} {...link('/roadmap')}>
					Roadmap
				</Dropdown.Item>
				<Dropdown.Item icon={User} {...link('/team')}>
					Team
				</Dropdown.Item>
				{/* <Dropdown.Item icon={Money} {...link('/pricing', router)}>
					Pricing
				</Dropdown.Item> */}
				<Dropdown.Item icon={Chat} {...link('/blog')}>
					Blog
				</Dropdown.Item>
				<Dropdown.Item icon={Book} {...link('/docs/product/getting-started/introduction')}>
					Docs
				</Dropdown.Item>
				<Dropdown.Item icon={Academia} {...link('/careers')}>
					Careers
					{positions.length > 0 ? (
						<span className="ml-2 rounded-md bg-primary px-[5px] py-px text-xs">
							{positions.length}
						</span>
					) : null}
				</Dropdown.Item>
			</Dropdown.Section>
		</Dropdown.Root>
	);
}

function rawLink(path: string, router: AppRouterInstance, pathname: string) {
	const selected = pathname.includes(path);

	return {
		selected,
		onClick: () => router.push(path),
		className: clsx(selected && 'bg-accent/20')
	};
}
