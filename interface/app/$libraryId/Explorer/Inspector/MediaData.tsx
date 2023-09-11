import { MediaLocation, MediaMetadata, MediaTime, Orientation } from '@sd/client';
import Accordion from '~/components/Accordion';
import { usePlatform } from '~/util/Platform';

import { MetaData } from './index';

interface Props {
	data: MediaMetadata;
}

function formatMediaTime(location: MediaTime): string | null {
	if (location === 'Undefined') return null;
	if ('Utc' in location) return location.Utc;
	if ('Naive' in location) return location.Naive;
	return null;
}

function formatLocationDD(location: MediaLocation, dp: number): string {
	return `${location.latitude.toFixed(dp)}, ${location.longitude.toFixed(dp)}`;
}

function formatLocationDMS(location: MediaLocation): string | null {
	const lat_abs = Math.abs(getZeroWithDecimals(location.latitude));
	const lat_d = flatTruncate(location.latitude, 0);
	const lat_m = flatTruncate(60 * lat_abs, 0);
	const lat_s = flatTruncate(lat_abs * 3600 - lat_m * 60, 0);
	const lat_sign = location.latitude > 0 ? 'N' : 'S';

	const long_abs = Math.abs(getZeroWithDecimals(location.latitude));
	const long_d = flatTruncate(location.latitude, 0);
	const long_m = flatTruncate(60 * long_abs, 0);
	const long_s = flatTruncate(long_abs * 3600 - long_m * 60, 0);
	const long_sign = location.latitude > 0 ? 'N' : 'S';

	return `${lat_d}° ${lat_m}' ${lat_s}" ${lat_sign}, ${long_d}° ${long_m}' ${long_s}" ${long_sign}`;
}

// Truncate without rounding. The easiest way to do this is by treating it as a string, as there
// will always be *some* edge-case when it comes to floating point arithmetic.
function flatTruncate(num: number, places: number): number {
	if (places > 8) return 0;
	const x = num.toString();
	const pos = x.indexOf('.');
	return pos !== -1
		? Number.parseFloat(
				`${x.substring(0, pos)}${places != 0 ? '.' + x.substring(pos, places) : ''}`
		  )
		: 0;
}

function getZeroWithDecimals(num: number): number {
	const x = num.toString();
	const pos = x.indexOf('.');
	return pos !== -1 ? Number.parseFloat(`${'0.' + x.substring(pos + 1)}`) : 0;
}

// return ~~(num * x) / x;

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
												`${data.location.latitude},${data.location.longitude}`
											)}}`
										);
								}}
							>
								{formatLocationDD(data.location, 3)}
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
