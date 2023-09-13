import { MediaLocation, MediaMetadata, MediaTime, Orientation } from '@sd/client';

import Accordion from '~/components/Accordion';
import { usePlatform } from '~/util/Platform';
import { MetaData } from './index';

interface Props {
	data: MediaMetadata;
}

function formatMediaTime(loc: MediaTime): string | null {
	if (loc === 'Undefined') return null;
	if ('Utc' in loc) return loc.Utc;
	if ('Naive' in loc) return loc.Naive;
	return null;
}

function formatLocation(loc: MediaLocation, dp: number): string {
	return `${loc.latitude.toFixed(dp)}, ${loc.longitude.toFixed(dp)}`;
}

const orientations = {
	Normal: 'Normal',
	MirroredHorizontal: 'Horizontally mirrored',
	MirroredHorizontalAnd90CW: 'Mirrored horizontally and rotated 90° clockwise',
	MirroredHorizontalAnd270CW: 'Mirrored horizontally and rotated 270° clockwise',
	MirroredVertical: 'Vertically mirrored',
	CW90: 'Rotated 90° clockwise',
	CW180: 'Rotated 180° clockwise',
	CW270: 'Rotated 270° clockwise'
};

function MediaData({ data }: Props) {
	const platform = usePlatform();

	return data.type === 'Image' ? (
		<div className="flex flex-col gap-0 py-2">
			<Accordion variant="apple" title="More info">
				<MetaData label="Date" value={formatMediaTime(data.date_taken)} />
				<MetaData label="Type" value={data.type} />
				<MetaData
					label="Location"
					tooltipValue={
						data.location
							? `${data.location.latitude}, ${data.location.longitude}`
							: '--'
					}
					value={
						data.location ? (
							<a
								onClick={(e) => {
									e.preventDefault();
									if (data.location)
										platform.openLink(
											`https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(
												data.location.latitude
											)}%2c${encodeURIComponent(data.location.longitude)}`
										);
								}}
							>
								{formatLocation(data.location, 3)}
							</a>
						) : (
							'--'
						)
					}
				/>
				<MetaData
					label="Plus Code"
					value={
						data.location?.pluscode ? (
							<a
								onClick={(e) => {
									e.preventDefault();
									if (data.location)
										platform.openLink(
											`https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(
												data.location.pluscode
											)}`
										);
								}}
							>
								{data.location?.pluscode}
							</a>
						) : (
							'--'
						)
					}
				/>
				<MetaData
					label="Dimensions"
					value={
						<>
							{data.dimensions.width} x {data.dimensions.height}
						</>
					}
				/>
				<MetaData label="Device" value={data.camera_data.device_make} />
				<MetaData label="Model" value={data.camera_data.device_model} />
				<MetaData
					label="Orientation"
					value={orientations[data.camera_data.orientation] ?? '--'}
				/>
				<MetaData label="Color profile" value={data.camera_data.color_profile} />
				<MetaData label="Color space" value={data.camera_data.color_space} />
				<MetaData label="Flash" value={data.camera_data.flash?.mode} />
				<MetaData label="Zoom" value={data.camera_data.zoom?.toFixed(2)} />
				<MetaData label="Iso" value={data.camera_data.iso} />
				<MetaData label="Software" value={data.camera_data.software} />
			</Accordion>
		</div>
	) : null;
}

export default MediaData;
