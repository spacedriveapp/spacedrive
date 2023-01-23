---
index: 10
---

# Database

### Schema

Our data schema is defined using Prisma, you can [view it on GitHub](https://github.com/spacedriveapp/spacedrive/blob/main/core/prisma/schema.prisma).
![A cool screenshot of the Spacedrive schema](/schema.webp) 

### Prisma Client Rust
We use Prisma Client Rust as a database ORM, it allows us to use Prisma to define our schema and generate migrations based on modifications to that schema. 
### Migrations
Migrations are run by the Prisma migration engine on app launch.
### Database file
The databases file is SQLite and can be opened in any SQL viewer.

![A Spacedrive library database file open in Table Plus](/database-table-plus.webp) 
