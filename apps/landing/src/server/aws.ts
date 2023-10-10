import { SendEmailCommand, SESClient } from '@aws-sdk/client-ses';
import { env } from '~/env';

export const ses = new SESClient({
	region: env.AWS_SES_REGION,
	credentials: {
		accessKeyId: env.AWS_SES_ACCESS_KEY,
		secretAccessKey: env.AWS_SES_SECRET_KEY
	}
});

export async function sendEmail(email: string, subject: string, body: string) {
	await ses.send(
		new SendEmailCommand({
			Destination: {
				ToAddresses: [email]
			},
			Message: {
				Body: {
					Html: {
						Charset: 'UTF-8',
						Data: body
					}
				},
				Subject: {
					Charset: 'UTF-8',
					Data: subject
				}
			},
			Source: env.MAILER_FROM
		})
	);
}
