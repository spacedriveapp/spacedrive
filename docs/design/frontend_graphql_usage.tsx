// Example: How the frontend would use the GraphQL API with full type safety

import { useQuery, useMutation, gql } from '@apollo/client';
import type { 
  Library, 
  CreateLibraryInput,
  UpdateLibrarySettingsInput 
} from './generated/graphql';

// GraphQL queries and mutations
const GET_LIBRARIES = gql`
  query GetLibraries {
    libraries {
      id
      name
      path
      description
      totalFiles
      totalSize
      createdAt
      updatedAt
    }
  }
`;

const CREATE_LIBRARY = gql`
  mutation CreateLibrary($input: CreateLibraryInput!) {
    createLibrary(input: $input) {
      id
      name
      path
      description
    }
  }
`;

const UPDATE_LIBRARY_SETTINGS = gql`
  mutation UpdateLibrarySettings($input: UpdateLibrarySettingsInput!) {
    updateLibrarySettings(input: $input) {
      id
      name
    }
  }
`;

// React component with full type safety
export function LibraryManager() {
  // Fully typed query - TypeScript knows the shape of data
  const { data, loading, error } = useQuery<{ libraries: Library[] }>(GET_LIBRARIES);
  
  // Fully typed mutation
  const [createLibrary] = useMutation<
    { createLibrary: Library },
    { input: CreateLibraryInput }
  >(CREATE_LIBRARY);
  
  const [updateSettings] = useMutation<
    { updateLibrarySettings: Library },
    { input: UpdateLibrarySettingsInput }
  >(UPDATE_LIBRARY_SETTINGS);
  
  const handleCreateLibrary = async () => {
    // TypeScript enforces correct input shape
    const result = await createLibrary({
      variables: {
        input: {
          name: "My Photos",
          description: "Personal photo collection",
          location: "/Users/me/Pictures"
        }
      }
    });
    
    // result.data.createLibrary is fully typed as Library
    console.log("Created library:", result.data?.createLibrary.name);
  };
  
  const handleUpdateSettings = async (libraryId: string) => {
    // TypeScript ensures we pass valid settings
    await updateSettings({
      variables: {
        input: {
          id: libraryId,
          generateThumbnails: true,
          thumbnailQuality: 90,
          enableAiTagging: false
        }
      }
    });
  };
  
  if (loading) return <div>Loading...</div>;
  if (error) return <div>Error: {error.message}</div>;
  
  return (
    <div>
      <h1>Libraries</h1>
      {data?.libraries.map(library => (
        <div key={library.id}>
          <h2>{library.name}</h2>
          <p>Files: {library.totalFiles}</p>
          <p>Size: {library.totalSize}</p>
          {/* TypeScript knows all available fields */}
          <button onClick={() => handleUpdateSettings(library.id)}>
            Update Settings
          </button>
        </div>
      ))}
      <button onClick={handleCreateLibrary}>Create Library</button>
    </div>
  );
}

// Example: Using with React hooks for even better DX
import { useGetLibrariesQuery, useCreateLibraryMutation } from './generated/graphql';

export function LibraryManagerWithHooks() {
  // Even simpler with generated hooks!
  const { data, loading } = useGetLibrariesQuery();
  const [createLibrary] = useCreateLibraryMutation();
  
  // Full intellisense and type checking
  const libraries = data?.libraries ?? [];
  
  return (
    <div>
      {libraries.map(lib => (
        <div key={lib.id}>{lib.name}</div>
      ))}
    </div>
  );
}