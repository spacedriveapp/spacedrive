import dayjs from 'dayjs';
import {
	capitalize,
	CoordinatesFormat,
	ExifMetadata,
	FFmpegMetadata,
	humanizeSize,
	int32ArrayToBigInt,
	MediaLocation,
	MediaData as RemoteMediaData,
	useSelector,
	useUnitFormatStore
} from '@sd/client';
import { Accordion } from '~/components';
import { useLocale } from '~/hooks';
import { Platform, usePlatform } from '~/util/Platform';

import { explorerStore } from '../store';
import { MetaData } from './index';

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

const ExifMediaData = (data: ExifMetadata) => {
	const platform = usePlatform();
	const { t } = useLocale();
	const coordinatesFormat = useUnitFormatStore().coordinatesFormat;
	const showMoreInfo = useSelector(explorerStore, (s) => s.showMoreInfo);

	return (
		<>
			<MetaData
				label={t('date')}
				tooltipValue={data.date_taken ?? null} // should show full raw value
				// should show localised, utc-offset value or plain value with tooltip mentioning that we don't have the timezone metadata
				value={data.date_taken ?? null}
			/>
			<MetaData label={t('type')} value={t('image')} />
			<MetaData
				label={t('location')}
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
				label={t('resolution')}
				value={`${data.resolution.width} x ${data.resolution.height}`}
			/>
			<MetaData label={t('device')} value={data.camera_data.device_make} />
			<MetaData label={t('model')} value={data.camera_data.device_model} />
			<MetaData label={t('color_profile')} value={data.camera_data.color_profile} />
			<MetaData label={t('color_space')} value={data.camera_data.color_space} />
			<MetaData
				label={t('flash')}
				value={t(`${data.camera_data.flash?.mode.toLowerCase()}`)}
			/>
			<MetaData
				label={t('zoom')}
				value={
					data.camera_data &&
					data.camera_data.zoom &&
					!Number.isNaN(data.camera_data.zoom)
						? `${data.camera_data.zoom.toFixed(2) + 'x'}`
						: '--'
				}
			/>
			<MetaData label="ISO" value={data.camera_data.iso} />
			<MetaData label={t('software')} value={data.camera_data.software} />
		</>
	);
};

const FFmpegMediaData = (data: FFmpegMetadata) => {
	const { t } = useLocale();

	const streamKinds = new Set(
		data.programs.flatMap((program) => program.streams.map((stream) => stream.codec?.kind))
	);
	const type = streamKinds.has('video')
		? 'Video'
		: streamKinds.has('audio')
			? 'Audio'
			: capitalize(streamKinds.values().next().value) ?? 'Unknown';

	const bit_rate = humanizeSize(int32ArrayToBigInt(data.bit_rate), {
		is_bit: true,
		base_unit: 'binary',
		use_plural: false
	});

	const duration_ms = data.duration ? int32ArrayToBigInt(data.duration) / 1000n : null;
	const duration = duration_ms
		? dayjs.duration(
				Number(duration_ms / 1000n) + Number(duration_ms % 1000n) / 1000,
				'seconds'
			)
		: null;

	const start_time_ms = data.start_time ? int32ArrayToBigInt(data.start_time) / 1000n : null;
	const start_time = start_time_ms
		? dayjs.duration(
				Number(start_time_ms / 1000n) + Number(start_time_ms % 1000n) / 1000,
				'seconds'
			)
		: null;

	const chapters = data.chapters
		.map((chapter) => {
			const num = BigInt(chapter.time_base_num);
			const den = BigInt(chapter.time_base_den);

			const start = dayjs.duration(
				Number((int32ArrayToBigInt(chapter.start) * num) / den),
				'seconds'
			);

			const end = dayjs.duration(
				Number((int32ArrayToBigInt(chapter.end) * num) / den),
				'seconds'
			);

			return `${start.format('HH:mm:ss')} - ${end.format('HH:mm:ss')}`;
		})
		.join('\n');

	return (
		<>
			<MetaData label={t('type')} value={type} />
			<MetaData label={t('bitrate')} value={`${bit_rate.value} ${bit_rate.unit}/s`} />
			{duration && <MetaData label={t('duration')} value={duration.format('HH:mm:ss.SSS')} />}
			{start_time && (
				<MetaData label={t('start_time')} value={start_time.format('HH:mm:ss.SSS')} />
			)}
			{chapters && <MetaData label={t('chapters')} value={chapters} />}
		</>
	);
};

interface Props {
	data: RemoteMediaData;
}

export const MediaData = ({ data }: Props) => {
	const { t } = useLocale();
	const showMoreInfo = useSelector(explorerStore, (s) => s.showMoreInfo);

	return (
		<div className="flex flex-col gap-0 py-2">
			<Accordion
				isOpen={showMoreInfo}
				onToggle={(isOpen) => (explorerStore.showMoreInfo = isOpen)}
				variant="apple"
				title={t('more_info')}
			>
				{'Exif' in data ? ExifMediaData(data.Exif) : FFmpegMediaData(data.FFmpeg)}
			</Accordion>
		</div>
	);
};

export default MediaData;
