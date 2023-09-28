import dayjs from 'dayjs';
import customParseFormat from 'dayjs/plugin/customParseFormat'; // import plugin
import {
	CoordinatesFormat,
	MediaDate,
	MediaLocation,
	MediaMetadata,
	useUnitFormatStore
} from '@sd/client';
import Accordion from '~/components/Accordion';
import { Platform, usePlatform } from '~/util/Platform';

import { getExplorerStore, useExplorerStore } from '../store';
import { MetaData } from './index';

interface Props {
	data: MediaMetadata;
}

// type MediaDateIntersection<U> = (U extends any ? (k: U) => void : never) extends (
// 	k: infer I
// ) => void
// 	? I
// 	: never;

// type MediaDateValue<U> = MediaDateIntersection<U> extends infer O
// 	? { [K in keyof O]: O[K] }
// 	: never;

const DateFormatWithTz = 'YYYY-MM-DD HH-MM-SS ZZ';
const DateFormatWithoutTz = 'YYYY-MM-DD HH-MM-SS';

const formatMediaDate = (
	datetime: MediaDate
): { formatted: string | undefined; raw: string } | undefined => {
	if (datetime === undefined) return undefined;
	dayjs.extend(customParseFormat);

	if (datetime)
		// const withTz = dayjs(datetime, );
		// console.log(datetime);

		// console.log(datetime);

		// return (
		// 	{
		// 		formatted: dayjs(dtType, FormatWithTz, true).format('YYYY-MM-DD HH:mm:ss'),
		// 		raw: dtType
		// 	} ?? {
		// 		formatted: dayjs(dtType, FormatWithoutTz, true).format('YYYY-MM-DD HH:mm:ss'),
		// 		raw: dtType
		// 	}
		// );

		return undefined;

	// return 'Utc' in datetime
	// ? {
	// 		formatted: dayjs(datetime.Utc, FormatWithTz, true).format('YYYY-MM-DD HH:mm:ss'),
	// 		raw: datetime.Utc
	//   }
	// 	: 'Naive' in datetime
	// 	? console.log(datetime.Naive)! && {
	// 			// ? dayjs(datetime.Naive, FormatWithoutTz).format('YYYY-MM-DD HH:mm:ss')
	// 		formatted: dayjs(datetime.Naive, FormatWithoutTz, true).format(
	// 			'YYYY-MM-DD HH:mm:ss'
	// 		),
	// 		raw: datetime.Naive
	//   }
	// 	: undefined;
	// if ('Utc' in datetime) return dayjs(datetime.Utc).format(FormatWithTz);
	// if ('Naive' in datetime) return dayjs(datetime.Naive).format(FormatWithoutTz);
	// if (datetime.t === 'Utc') return dayjs(datetime.c, FormatWithTz).format('MMM do YYYY');
	// if (datetime.t === 'Naive') return dayjs(datetime.c, FormatWithoutTz).format('MMM do YYYY');
};

const formatLocationDD = (loc: MediaLocation, dp?: number): string => {
	// the lack of a + here will mean that coordinates may have padding at the end
	// google does the same (if one is larger than the other, the smaller one will be padded with zeroes)
	return `${loc.latitude.toFixed(dp ?? 8)}, ${loc.longitude.toFixed(dp ?? 8)}`;
};

const formatLocationDMS = (loc: MediaLocation, dp?: number): string => {
	const formatCoordinatesAsDMS = (
		coordinates: number,
		positiveChar: string,
		negativeChar: string
	): string => {
		const abs = getAbsoluteDecimals(coordinates);
		const d = Math.trunc(coordinates);
		const m = Math.trunc(60 * abs);
		// adding 0.05 before rounding and truncating with `toFixed` makes it match up with google
		const s = (abs * 3600 - m * 60 + 0.05).toFixed(dp ?? 1);
		const sign = coordinates > 0 ? positiveChar : negativeChar;
		return `${d}°${m}'${s}"${sign}`;
	};

	return `${formatCoordinatesAsDMS(loc.latitude, 'N', 'S')} ${formatCoordinatesAsDMS(
		loc.longitude,
		'E',
		'W'
	)}`;
};

const getAbsoluteDecimals = (num: number): number => {
	const x = num.toString();
	// this becomes +0.xxxxxxxxx and is needed to convert the minutes/seconds for DMS
	return Math.abs(Number.parseFloat('0.' + x.substring(x.indexOf('.') + 1)));
};

const formatLocation = (loc: MediaLocation, format: CoordinatesFormat, dp?: number): string => {
	return format === 'dd' ? formatLocationDD(loc, dp) : formatLocationDMS(loc, dp);
};

const UrlMetadataValue = (props: { text: string; url: string; platform: Platform }) => (
	<a
		onClick={(e) => {
			e.preventDefault();
			props.platform.openLink(props.url);
		}}
	>
		{props.text}
	</a>
);

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

const MediaData = ({ data }: Props) => {
	const platform = usePlatform();
	const coordinatesFormat = useUnitFormatStore().coordinatesFormat;

	const explorerStore = useExplorerStore();

	return data.type === 'Image' ? (
		<div className="flex flex-col gap-0 py-2">
			<Accordion
				isOpen={explorerStore.showMoreInfo}
				onToggle={(isOpen) => (getExplorerStore().showMoreInfo = isOpen)}
				variant="apple"
				title="More info"
			>
				<MetaData
					label="Date"
					tooltipValue={data.date_taken && formatMediaDate(data.date_taken)?.raw}
					value={data.date_taken && formatMediaDate(data.date_taken)?.formatted}
				/>
				<MetaData label="Type" value={data.type} />
				<MetaData
					label="Location"
					tooltipValue={data.location && formatLocation(data.location, coordinatesFormat)}
					value={
						data.location && (
							<UrlMetadataValue
								url={`https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(
									formatLocation(data.location, 'dd')
								)}`}
								text={formatLocation(
									data.location,
									coordinatesFormat,
									coordinatesFormat === 'dd' ? 4 : 0
								)}
								platform={platform}
							/>
						)
					}
				/>
				<MetaData
					label="Plus Code"
					value={
						data.location?.pluscode && (
							<UrlMetadataValue
								url={`https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(
									data.location.pluscode
								)}`}
								text={data.location.pluscode}
								platform={platform}
							/>
						)
					}
				/>
				<MetaData
					label="Resolution"
					value={`${data.resolution.width} x ${data.resolution.height}`}
				/>
				<MetaData label="Device" value={data.camera_data.device_make} />
				<MetaData label="Model" value={data.camera_data.device_model} />
				<MetaData label="Color profile" value={data.camera_data.color_profile} />
				<MetaData label="Color space" value={data.camera_data.color_space} />
				<MetaData label="Flash" value={data.camera_data.flash?.mode} />
				<MetaData
					label="Zoom"
					value={
						data.camera_data &&
						data.camera_data.zoom &&
						!Number.isNaN(data.camera_data.zoom)
							? `${data.camera_data.zoom.toFixed(2) + 'x'}`
							: '--'
					}
				/>
				<MetaData label="Iso" value={data.camera_data.iso} />
				<MetaData label="Software" value={data.camera_data.software} />
			</Accordion>
		</div>
	) : null;
};

export default MediaData;
