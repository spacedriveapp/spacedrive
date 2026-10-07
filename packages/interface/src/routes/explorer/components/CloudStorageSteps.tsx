import { useForm } from "react-hook-form";
import { CloudArrowUp } from "@phosphor-icons/react";
import { Input, Label } from "@spacedrive/primitives";
import type {
	CloudServiceType,
	CloudStorageConfig,
	SdPath,
	VolumeAddCloudInput,
} from "@sd/ts-client";
import clsx from "clsx";
import { useLibraryMutation } from "../../../contexts/SpacedriveContext";
import { describeError } from "../../../components/modals/RearrangeModal";

import DriveAmazonS3 from "@sd/assets/icons/Drive-AmazonS3.webp";
import DriveGoogleDrive from "@sd/assets/icons/Drive-GoogleDrive.webp";
import DriveDropbox from "@sd/assets/icons/Drive-Dropbox.webp";
import DriveOneDrive from "@sd/assets/icons/Drive-OneDrive.webp";
import DriveBackBlaze from "@sd/assets/icons/Drive-BackBlaze.webp";
import DrivePCloud from "@sd/assets/icons/Drive-PCloud.webp";
import DriveBox from "@sd/assets/icons/Drive-Box.webp";

/**
 * A cloud volume is an adapter origin, not a walked root: connecting one
 * registers and tracks it in one call, so these steps precede Add to Library
 * rather than ending in its setup.
 */

export interface CloudProvider {
	id: CloudServiceType | "r2" | "minio";
	name: string;
	icon: string;
	cloudServiceType: CloudServiceType;
}

export const cloudProviders: CloudProvider[] = [
	{ id: "s3", name: "Amazon S3", icon: DriveAmazonS3, cloudServiceType: "s3" },
	{ id: "r2", name: "Cloudflare R2", icon: DriveAmazonS3, cloudServiceType: "s3" },
	{ id: "minio", name: "MinIO", icon: DriveAmazonS3, cloudServiceType: "s3" },
	{ id: "b2", name: "Backblaze B2", icon: DriveBackBlaze, cloudServiceType: "b2" },
	{ id: "wasabi", name: "Wasabi", icon: DriveAmazonS3, cloudServiceType: "wasabi" },
	{ id: "spaces", name: "DO Spaces", icon: DriveAmazonS3, cloudServiceType: "spaces" },
	{ id: "gdrive", name: "Google Drive", icon: DriveGoogleDrive, cloudServiceType: "gdrive" },
	{ id: "dropbox", name: "Dropbox", icon: DriveDropbox, cloudServiceType: "dropbox" },
	{ id: "onedrive", name: "OneDrive", icon: DriveOneDrive, cloudServiceType: "onedrive" },
	{ id: "gcs", name: "Google Cloud", icon: DriveGoogleDrive, cloudServiceType: "gcs" },
	{ id: "azblob", name: "Azure Blob", icon: DriveBox, cloudServiceType: "azblob" },
	{ id: "cloud", name: "pCloud", icon: DrivePCloud, cloudServiceType: "cloud" },
];

export const cloudStepIcon = <CloudArrowUp size={20} weight="fill" />;

export function CloudProviderGrid({
	onSelect,
}: {
	onSelect: (provider: CloudProvider) => void;
}) {
	return (
		<div className="grid max-h-[400px] grid-cols-3 gap-3 overflow-y-auto pr-1">
			{cloudProviders.map((provider) => (
				<button
					key={provider.id}
					type="button"
					onClick={() => onSelect(provider)}
					className={clsx(
						"flex flex-col items-center gap-2 rounded-lg border p-4",
						"transition-all hover:scale-[1.02]",
						"border-app-line bg-app-box hover:border-accent/50 hover:bg-app-hover",
					)}
				>
					<img src={provider.icon} className="size-10" alt="" />
					<div className="text-center text-xs font-medium text-ink">
						{provider.name}
					</div>
				</button>
			))}
		</div>
	);
}

export interface CloudFormData {
	display_name: string;
	bucket?: string;
	region?: string;
	access_key_id?: string;
	secret_access_key?: string;
	endpoint?: string;
	access_token?: string;
	refresh_token?: string;
	client_id?: string;
	client_secret?: string;
	root?: string;
	container?: string;
	account_name?: string;
	account_key?: string;
	credential?: string;
}

function isS3Like(service: CloudServiceType) {
	return service === "s3" || service === "b2" || service === "wasabi" || service === "spaces";
}

function isOAuth(service: CloudServiceType) {
	return service === "gdrive" || service === "dropbox" || service === "onedrive";
}

function cloudConfig(provider: CloudProvider, data: CloudFormData): CloudStorageConfig {
	const service = provider.cloudServiceType;
	if (isS3Like(service)) {
		return {
			type: "S3",
			bucket: data.bucket ?? "",
			region: data.region ?? "",
			access_key_id: data.access_key_id ?? "",
			secret_access_key: data.secret_access_key ?? "",
			endpoint: data.endpoint || null,
		};
	}
	if (service === "dropbox") {
		return {
			type: "Dropbox",
			root: data.root || null,
			refresh_token: data.refresh_token ?? "",
			client_id: data.client_id ?? "",
			client_secret: data.client_secret ?? "",
		};
	}
	if (service === "gdrive" || service === "onedrive") {
		return {
			type: service === "gdrive" ? "GoogleDrive" : "OneDrive",
			root: data.root || null,
			access_token: data.access_token ?? "",
			refresh_token: data.refresh_token ?? "",
			client_id: data.client_id ?? "",
			client_secret: data.client_secret ?? "",
		};
	}
	if (service === "azblob") {
		return {
			type: "AzureBlob",
			container: data.container ?? "",
			endpoint: data.endpoint || null,
			account_name: data.account_name ?? "",
			account_key: data.account_key ?? "",
		};
	}
	if (service === "gcs") {
		return {
			type: "GoogleCloudStorage",
			bucket: data.bucket ?? "",
			root: data.root || null,
			endpoint: data.endpoint || null,
			credential: data.credential ?? "",
		};
	}
	throw new Error(`${provider.name} is not supported yet`);
}

function cloudIdentifier(provider: CloudProvider, data: CloudFormData): string {
	const service = provider.cloudServiceType;
	if (isS3Like(service) || service === "gcs") return data.bucket ?? "";
	if (service === "azblob") return data.container ?? "";
	if (isOAuth(service)) return data.root || "root";
	return "root";
}

/** The connection form for one provider, owned by the dialog that hosts it. */
export function useCloudConnection(
	provider: CloudProvider | null,
	onConnected: (root: SdPath) => void,
) {
	const form = useForm<CloudFormData>({ defaultValues: { display_name: "" } });
	const addCloudVolume = useLibraryMutation("volumes.add_cloud");

	const submit = form.handleSubmit(async (data) => {
		if (!provider) return;
		try {
			const input: VolumeAddCloudInput = {
				service: provider.cloudServiceType,
				display_name: data.display_name,
				config: cloudConfig(provider, data),
			};
			await addCloudVolume.mutateAsync(input);
			onConnected({
				Cloud: {
					service: provider.cloudServiceType,
					identifier: cloudIdentifier(provider, data),
					path: "",
				},
			});
		} catch (error) {
			form.setError("root", { type: "manual", message: describeError(error) });
		}
	});

	return { form, submit, pending: addCloudVolume.isPending };
}

export function CloudConnectionFields({
	provider,
	form,
}: {
	provider: CloudProvider;
	form: ReturnType<typeof useForm<CloudFormData>>;
}) {
	const service = provider.cloudServiceType;
	const field = (
		name: keyof CloudFormData,
		label: string,
		props: { placeholder?: string; type?: string } = {},
	) => (
		<div className="space-y-2">
			<Label>{label}</Label>
			<Input
				{...form.register(name)}
				size="md"
				className="bg-app-input"
				{...props}
			/>
		</div>
	);

	return (
		<div className="h-full space-y-4 overflow-y-auto pr-1">
			{field("display_name", "Display Name", { placeholder: `My ${provider.name}` })}

			{isS3Like(service) && (
				<>
					{field("bucket", "Bucket", { placeholder: "my-bucket" })}
					{field("region", "Region", { placeholder: "us-west-2" })}
					{field("access_key_id", "Access Key ID", { placeholder: "AKIA..." })}
					{field("secret_access_key", "Secret Access Key", {
						type: "password",
						placeholder: "••••••••••••••••••",
					})}
					{(provider.id === "r2" ||
						provider.id === "minio" ||
						provider.id === "wasabi" ||
						provider.id === "spaces") &&
						field("endpoint", "Endpoint", {
							placeholder:
								provider.id === "r2"
									? "https://account.r2.cloudflarestorage.com"
									: provider.id === "minio"
										? "http://localhost:9000"
										: "https://...",
						})}
				</>
			)}

			{isOAuth(service) && (
				<>
					{field("client_id", "Client ID")}
					{field("client_secret", "Client Secret", { type: "password" })}
					{service !== "dropbox" && field("access_token", "Access Token")}
					{field("refresh_token", "Refresh Token")}
					{field("root", "Root Path (Optional)", { placeholder: "/" })}
				</>
			)}

			{service === "azblob" && (
				<>
					{field("container", "Container", { placeholder: "my-container" })}
					{field("account_name", "Account Name")}
					{field("account_key", "Account Key", { type: "password" })}
					{field("endpoint", "Endpoint (Optional)", { placeholder: "https://..." })}
				</>
			)}

			{service === "gcs" && (
				<>
					{field("bucket", "Bucket", { placeholder: "my-gcs-bucket" })}
					<div className="space-y-2">
						<Label>Service Account Credential (JSON)</Label>
						<textarea
							{...form.register("credential")}
							rows={6}
							placeholder='{"type": "service_account", ...}'
							className="w-full rounded-lg border border-app-line bg-app-input px-3 py-2 font-mono text-sm text-ink"
						/>
					</div>
					{field("root", "Root Path (Optional)", { placeholder: "/" })}
					{field("endpoint", "Endpoint (Optional)", {
						placeholder: "https://storage.googleapis.com",
					})}
				</>
			)}

			{form.formState.errors.root && (
				<p className="text-xs text-red-500">{form.formState.errors.root.message}</p>
			)}
		</div>
	);
}
