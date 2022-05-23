import type { File } from "./File";
import type { FilePath } from "./FilePath";

export interface DirectoryWithContents { directory: FilePath, contents: Array<File>, }