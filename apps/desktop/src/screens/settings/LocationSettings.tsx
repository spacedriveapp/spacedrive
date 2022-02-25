import React from 'react';
import { Button } from '../../components/primitive';
import { useLocations } from '../../store/locations';
import Listbox from '../../components/primitive/Listbox';
import { InputContainer } from '../../components/primitive/InputContainer';

const exampleLocations = [
  { option: 'Macintosh HD', key: 'macintosh_hd' },
  { option: 'Lacie External', key: 'lacie_external' },
  { option: 'Seagate 8TB', key: 'seagate_8tb' }
];

export default function LocationSettings() {
  const locations = useLocations();

  return (
    <div className="max-w-md">
      {/*<Button size="sm">Add Location</Button>*/}
      <InputContainer
        title="Something about a vault"
        description="Local cache storage for media previews and thumbnails."
      >
        <div className="flex flex-row space-x-2">
          <div className="flex flex-grow">
            <Listbox
              options={locations.map((location) => ({
                key: location.name,
                option: location.name,
                description: location.path
              }))}
            />
          </div>
          <Button className="mb-3" variant="primary">
            Add Location
          </Button>
        </div>
      </InputContainer>
    </div>
  );
}
