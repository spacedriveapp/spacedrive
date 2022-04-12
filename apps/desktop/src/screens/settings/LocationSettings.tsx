import React from 'react';
import { Button } from '../../components/primitive';
import { Dropdown } from '../../components/primitive/Dropdown';
import DropdownMenu from '../../components/primitive/DropdownMenu';
import { InputContainer } from '../../components/primitive/InputContainer';

const exampleLocations = [
  { option: 'Macintosh HD', key: 'macintosh_hd' },
  { option: 'LaCie External', key: 'lacie_external' },
  { option: 'Seagate 8TB', key: 'seagate_8tb' }
];

export default function LocationSettings() {
  // const locations = useBridgeQuery("SysGetLocation")

  return (
    <div className="flex flex-col flex-grow max-w-4xl space-y-4">
      {/*<Button size="sm">Add Location</Button>*/}
      <InputContainer
        title="Something about a vault"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row space-x-2">
          <div className="flex flex-grow">
            {/* <Listbox
              options={locations.map((location) => ({
                key: location.name,
                option: location.name,
                description: location.path
              }))}
            /> */}
          </div>
          <Button className="mb-3" variant="primary">
            Add Location
          </Button>
          <DropdownMenu />
        </div>
      </InputContainer>
    </div>
  );
}
