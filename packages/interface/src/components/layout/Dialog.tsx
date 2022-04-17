// import { Button } from '../primitive';
// import React from 'react';
// import * as DialogPrimitive from '@radix-ui/react-dialog';
//
// export default function DialogButton<{ children: React.ReactNode }>(props) {
//   return (
//     <DialogPrimitive.Root>
//       <DialogPrimitive.Trigger asChild>
//         <Button variant="primary">Add Location</Button>
//       </DialogPrimitive.Trigger>
//       <DialogPrimitive.Portal className="">
//         <DialogPrimitive.Overlay className="bg-black bg-opacity-50 absolute top-0 left-0 h-screen w-screen" />
//         <DialogPrimitive.Content className="absolute p-8 rounded-md margin-auto bg-gray-700 text-white">
//           <DialogPrimitive.Title>Add Location</DialogPrimitive.Title>
//           <DialogPrimitive.Description>Choose a location from the list</DialogPrimitive.Description>
//           <div className="flex flex-row space-x-2 mt-1">
//             <DialogPrimitive.Close asChild>
//               <Button>Close</Button>
//             </DialogPrimitive.Close>
//             <Button variant="primary">Add Location</Button>
//           </div>
//         </DialogPrimitive.Content>
//       </DialogPrimitive.Portal>
//     </DialogPrimitive.Root>
//   );
// }
