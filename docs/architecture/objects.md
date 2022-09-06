# Objects

Objects are primarily created by the identifier from Paths. They can be created from any kind of file or directory. All metadata created around files in Spacedrive are directly attached to the Object for that file. 

A CAS id is generated from samples of the byte data, which is used to associate Objects uniquely with logical Paths found in a location.

Some Objects are purely virtual, meaning they have no Path and are likely only used in a Space.



## Types of object

| Name             | Description                                                  | Code |
| ---------------- | ------------------------------------------------------------ | ---- |
| Unknown          | A file that can not be identified by the indexer             | 0    |
| Document         | A known filetype, but without specific support               | 1    |
| Folder           | A virtual filesystem directory                               | 2    |
| Text File        | A file that contains human-readable text                     | 3    |
| Package          | A folder that opens an application                           | 4    |
| Image            | An image file                                                | 5    |
| Audio            | An audio file                                                | 6    |
| Video            | A video file                                                 | 7    |
| Archive          | A compressed archive of data                                 | 8    |
| Executable       | An executable program or application                         | 9    |
| Alias            | A link to another Object                                     | 10   |
| Encrypted Bytes  | Raw bytes with self contained metadata                       | 11   |
| Link             | A link to a web page, application or Space                   | 12   |
| Web Page Archive | A snapshot of a webpage, with HTML, JS, images and screenshot | 13   |
| Widget           | A widget is a mini app that can be placed in a Space at various sizes, associated Widget struct required | 14   |
| Album            | Albums can only have one level of children, and are associated with the Album struct | 15   |
| Collection       | Its like a folder, but appears like a stack of files, designed for burst photos/associated groups of files | 16   |

