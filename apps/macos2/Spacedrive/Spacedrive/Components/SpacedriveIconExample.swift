import SwiftUI

/// Example usage of the SpacedriveIcon component
struct SpacedriveIconExample: View {
    var body: some View {
        VStack(spacing: 20) {
            Text("Spacedrive Icon Examples")
                .font(.title)
                .padding()

            // Basic usage examples
            VStack(alignment: .leading, spacing: 12) {
                Text("Basic Usage")
                    .font(.headline)

                HStack(spacing: 16) {
                    SpacedriveIconView(.folder, size: 24)
                    SpacedriveIconView(.document, size: 24)
                    SpacedriveIconView(.image, size: 24)
                    SpacedriveIconView(.video, size: 24)
                    SpacedriveIconView(.audio, size: 24)
                }

                Text("Different Sizes")
                    .font(.headline)
                    .padding(.top)

                HStack(spacing: 16) {
                    SpacedriveIconView(.folder, size: 16)
                    SpacedriveIconView(.folder, size: 24)
                    SpacedriveIconView(.folder, size: 32)
                    SpacedriveIconView(.folder, size: 48)
                }

                Text("Light Theme Variants")
                    .font(.headline)
                    .padding(.top)

                HStack(spacing: 16) {
                    SpacedriveIconView(.folder, size: 24)
                    SpacedriveIconView(.folderLight, size: 24)
                    SpacedriveIconView(.document, size: 24)
                    SpacedriveIconView(.documentLight, size: 24)
                }

                Text("20px Variants")
                    .font(.headline)
                    .padding(.top)

                HStack(spacing: 16) {
                    SpacedriveIconView(.folder, size: 24)
                    SpacedriveIconView(.folder20, size: 24)
                    SpacedriveIconView(.image, size: 24)
                    SpacedriveIconView(.image20, size: 24)
                }

                Text("Drive/Cloud Services")
                    .font(.headline)
                    .padding(.top)

                HStack(spacing: 16) {
                    SpacedriveIconView(.drive, size: 24)
                    SpacedriveIconView(.driveDropbox, size: 24)
                    SpacedriveIconView(.driveGoogleDrive, size: 24)
                    SpacedriveIconView(.driveAmazonS3, size: 24)
                    SpacedriveIconView(.driveOneDrive, size: 24)
                }

                Text("Device Icons")
                    .font(.headline)
                    .padding(.top)

                HStack(spacing: 16) {
                    SpacedriveIconView(.laptop, size: 24)
                    SpacedriveIconView(.mobile, size: 24)
                    SpacedriveIconView(.tablet, size: 24)
                    SpacedriveIconView(.server, size: 24)
                    SpacedriveIconView(.hdd, size: 24)
                }
            }
            .padding()

            // Icon picker example
            VStack(alignment: .leading, spacing: 12) {
                Text("Icon Information")
                    .font(.headline)

                let sampleIcon = SpacedriveIcon.folder
                VStack(alignment: .leading, spacing: 4) {
                    Text("Icon: \(sampleIcon.displayName)")
                    Text("Filename: \(sampleIcon.fullFilename)")
                    Text("Base Name: \(sampleIcon.baseName)")
                    Text("Is Light Variant: \(sampleIcon.isLightVariant ? "Yes" : "No")")
                    Text("Is 20px Variant: \(sampleIcon.is20pxVariant ? "Yes" : "No")")
                }
                .font(.caption)
                .foregroundColor(.secondary)
            }
            .padding()
            .background(Color.gray.opacity(0.1))
            .cornerRadius(8)
            .padding()
        }
    }
}

/// Preview for the example
struct SpacedriveIconExample_Previews: PreviewProvider {
    static var previews: some View {
        SpacedriveIconExample()
            .frame(width: 600, height: 800)
    }
}
