---
name: Objects
index: 2
new: true
---

# Objects

Objects are files discovered on your devices and drives, but can also be virtual, existing only within Spacedrive.
 
All metadata associated with files in Spacedrive is linked to the Object for that file. 

:::warning
Spacedrive is under active development, most of the listed features are still experimental and subject to change.
:::
:::info
Spacedrive is under active development, most of the listed features are still experimental and subject to change.
:::
:::note
Spacedrive is under active development, most of the listed features are still experimental and subject to change.
:::
:::jeff
Spacedrive is under active development, most of the listed features are still experimental and subject to change.
:::

If two or more files are discovered that are identical, they are linked to the same Object in Spacedrive.

A CAS id is generated from set samples of the byte data, which is used to associate Objects uniquely with Paths found in a Location.

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