export * as SharedItems from './SharedItems';
export * as FilePathItems from './FilePath/Items';
export * as ObjectItems from './Object/Items';

import { ExplorerItem } from '@sd/client';
import FilePathCM from "./FilePath"
import ObjectCM from "./Object"
import LocationCM from "./Location"

export default ({ item }: { item: ExplorerItem }) => {
    switch (item.type) {
        case "Path":
            return <FilePathCM data={item} />
        case "Object":
            return <ObjectCM data={item} />
        case "Location":
            return <LocationCM data={item} />
    }
}
