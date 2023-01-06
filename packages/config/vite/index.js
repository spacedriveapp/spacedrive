const fs = require("fs/promises")
const path = require("path")

// only supports files rn
module.exports = {
	relativeAliasResolver: {
		find: /^(~\/.+)/,
		replacement: "$1",
		async customResolver(source, importer) {
			const [repo, filePath] = importer.split("/packages/");
			const [pkg] = filePath.split("/src/")

			const sourcePath = source.substring(2)

			const absolutePath = `${repo}/packages/${pkg}/src/${sourcePath}`;

			const folderItems = await fs.readdir(path.join(absolutePath, "../"));

			const item = folderItems.find(i => i.startsWith(sourcePath.split("/").at(-1)))

			return absolutePath + path.extname(item)
		}
	}
}
