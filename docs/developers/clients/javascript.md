---
name: JavaScript
index: 500
---

# JavaScript Client

For developers who want to extend the functionality of Spacedrive, this library allows easy development of [Extensions](), providing full access to Spacedrive's functionality.

### Installation

```shell
$ npm i @spacedrive/client
```

Initialize the Spacedrive client

```ts
import spacedrive from '@spacedrive/client';

spacedrive.start();
```

Add a location

```ts
const location = await spacedrive.location.create('/Users/jamie/Documents');

location.scan();
```
