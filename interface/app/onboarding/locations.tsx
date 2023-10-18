import {
	Desktop,
	DownloadSimple,
	File,
	Image,
	MusicNote,
	Icon as PhosportIcon,
	Video
} from '@phosphor-icons/react';
import clsx from 'clsx';
import { useMemo } from 'react';
import { Controller, useWatch } from 'react-hook-form';
import { useNavigate } from 'react-router';
import { SystemLocations, useBridgeQuery } from '@sd/client';
import { Button, Form, RadixCheckbox } from '@sd/ui';
import { Icon, TruncatedText } from '~/components';
import { useIsDark } from '~/hooks';

import { useOnboardingContext } from './context';
import { OnboardingContainer, OnboardingDescription, OnboardingTitle } from './Layout';

const icons: Record<keyof SystemLocations, PhosportIcon> = {
	desktop: Desktop,
	documents: File,
	downloads: DownloadSimple,
	pictures: Image,
	music: MusicNote,
	videos: Video,
	movies: Video
};

const LocationIcon = (props: { location: keyof SystemLocations; active?: boolean }) => {
	const isDark = useIsDark();

	const LocationIcon = icons[props.location];

	return (
		<div className="absolute -bottom-9 -right-9 h-28 w-28">
			<Icon name="Folder" />
			<LocationIcon
				weight="fill"
				size={28}
				className={clsx(
					'absolute left-1/2 top-[42%] -translate-x-1/2 fill-black transition-opacity duration-200',
					isDark
						? 'opacity-30 group-focus-within:opacity-60 group-hover:opacity-60'
						: 'opacity-25 group-focus-within:opacity-50 group-hover:opacity-50',
					props.active && (isDark ? 'opacity-60' : 'opacity-50')
				)}
			/>
		</div>
	);
};

export default function OnboardingLocations() {
	const navigate = useNavigate();

	const { data: systemLocations } = useBridgeQuery(['locations.systemLocations']);

	const form = useOnboardingContext().forms.useForm('locations');

	const locations = useWatch({ control: form.control, name: 'locations' });

	const toggled = useMemo(() => {
		const values = Object.values(locations);
		return values.length > 0 && !values.some((val) => !val);
	}, [locations]);

	return (
		<Form
			form={form}
			onSubmit={form.handleSubmit(() => navigate('../privacy', { replace: true }))}
			className="flex flex-col items-center"
		>
			<OnboardingContainer>
				<OnboardingTitle>Add Locations</OnboardingTitle>
				<OnboardingDescription>
					Enhance your Spacedrive experience by adding your favorite locations to your
					personal library, for seamless and efficient file management.
				</OnboardingDescription>

				{systemLocations && (
					<div className="my-6">
						<RadixCheckbox
							name="toggle-all"
							className="mb-1.5 justify-end"
							labelClassName="!ml-1.5"
							label="Toggle All"
							checked={toggled}
							onCheckedChange={(value) => {
								if (typeof value !== 'boolean') return;

								form.reset({
									locations: Object.fromEntries(
										Object.entries(systemLocations).map(([key, value]) => [
											key,
											Boolean(value)
										])
									)
								});
							}}
						/>

						<div className="grid grid-cols-2 gap-2">
							{(Object.keys(systemLocations) as (keyof typeof systemLocations)[]).map(
								(location) => {
									const path = systemLocations[location];
									if (!path) return null;

									return (
										<Controller
											key={location}
											control={form.control}
											name={`locations.${location}`}
											render={({ field }) => (
												<label
													htmlFor={field.name}
													className={clsx(
														'group relative flex w-72 overflow-hidden rounded-md border px-4 py-3',
														field.value
															? 'border-accent/25 bg-accent/10'
															: 'border-app-line bg-app-box/50'
													)}
												>
													<RadixCheckbox
														name={field.name}
														checked={field.value}
														onCheckedChange={field.onChange}
														className="mr-2 mt-1 self-start"
													/>

													<div className="max-w-[64%]">
														<h1 className="font-bold capitalize">
															{location}
														</h1>
														<TruncatedText className="text-sm text-ink-faint">
															{systemLocations[location]}
														</TruncatedText>
													</div>

													<LocationIcon
														location={location}
														active={field.value}
													/>
												</label>
											)}
										/>
									);
								}
							)}
						</div>
					</div>
				)}
				{/* TODO: Form error handling */}
				<Button type="submit" className="text-center" variant="accent" size="sm">
					Continue
				</Button>
			</OnboardingContainer>
		</Form>
	);
}
