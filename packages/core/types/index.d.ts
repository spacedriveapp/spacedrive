
/**
 * Client
**/

import * as runtime from './runtime/index';
declare const prisma: unique symbol
export type PrismaPromise<A> = Promise<A> & {[prisma]: true}
type UnwrapPromise<P extends any> = P extends Promise<infer R> ? R : P
type UnwrapTuple<Tuple extends readonly unknown[]> = {
  [K in keyof Tuple]: K extends `${number}` ? Tuple[K] extends PrismaPromise<infer X> ? X : UnwrapPromise<Tuple[K]> : UnwrapPromise<Tuple[K]>
};


/**
 * Model Migration
 * 
 */
export type Migration = {
  id: number
  name: string
  checksum: string
  steps_applied: number
  applied_at: Date
}

/**
 * Model Library
 * 
 */
export type Library = {
  id: number
  uuid: string
  name: string
  remote_id: string | null
  is_primary: boolean
  encryption: number
  date_created: Date
  timezone: string | null
}

/**
 * Model LibraryStatistics
 * 
 */
export type LibraryStatistics = {
  id: number
  date_captured: Date
  library_id: number
  total_file_count: number
  total_bytes_used: string
  total_byte_capacity: string
  total_unique_bytes: string
}

/**
 * Model Client
 * 
 */
export type Client = {
  id: number
  uuid: string
  name: string
  platform: number
  version: string | null
  online: boolean | null
  last_seen: Date
  timezone: string | null
  date_created: Date
}

/**
 * Model Location
 * 
 */
export type Location = {
  id: number
  name: string | null
  path: string | null
  total_capacity: number | null
  available_capacity: number | null
  is_removable: boolean
  is_ejectable: boolean
  is_root_filesystem: boolean
  is_online: boolean
  date_created: Date
}

/**
 * Model File
 * 
 */
export type File = {
  id: number
  is_dir: boolean
  location_id: number
  stem: string
  name: string
  extension: string | null
  quick_checksum: string | null
  full_checksum: string | null
  size_in_bytes: string
  encryption: number
  date_created: Date
  date_modified: Date
  date_indexed: Date
  ipfs_id: string | null
  parent_id: number | null
}

/**
 * Model Tag
 * 
 */
export type Tag = {
  id: number
  name: string | null
  encryption: number | null
  total_files: number | null
  redundancy_goal: number | null
  date_created: Date
  date_modified: Date
}

/**
 * Model TagOnFile
 * 
 */
export type TagOnFile = {
  date_created: Date
  tag_id: number
  file_id: number
}

/**
 * Model Job
 * 
 */
export type Job = {
  id: number
  client_id: number
  action: number
  status: number
  percentage_complete: number
  task_count: number
  completed_task_count: number
  date_created: Date
  date_modified: Date
}

/**
 * Model Space
 * 
 */
export type Space = {
  id: number
  name: string
  encryption: number | null
  date_created: Date
  date_modified: Date
  libraryId: number | null
}


/**
 * ##  Prisma Client ʲˢ
 * 
 * Type-safe database client for TypeScript & Node.js
 * @example
 * ```
 * const prisma = new PrismaClient()
 * // Fetch zero or more Migrations
 * const migrations = await prisma.migration.findMany()
 * ```
 *
 * 
 * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client).
 */
export class PrismaClient<
  T extends Prisma.PrismaClientOptions = Prisma.PrismaClientOptions,
  U = 'log' extends keyof T ? T['log'] extends Array<Prisma.LogLevel | Prisma.LogDefinition> ? Prisma.GetEvents<T['log']> : never : never,
  GlobalReject = 'rejectOnNotFound' extends keyof T
    ? T['rejectOnNotFound']
    : false
      > {
      /**
       * @private
       */
      private fetcher;
      /**
       * @private
       */
      private readonly dmmf;
      /**
       * @private
       */
      private connectionPromise?;
      /**
       * @private
       */
      private disconnectionPromise?;
      /**
       * @private
       */
      private readonly engineConfig;
      /**
       * @private
       */
      private readonly measurePerformance;

    /**
   * ##  Prisma Client ʲˢ
   * 
   * Type-safe database client for TypeScript & Node.js
   * @example
   * ```
   * const prisma = new PrismaClient()
   * // Fetch zero or more Migrations
   * const migrations = await prisma.migration.findMany()
   * ```
   *
   * 
   * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client).
   */

  constructor(optionsArg ?: Prisma.Subset<T, Prisma.PrismaClientOptions>);
  $on<V extends (U | 'beforeExit')>(eventType: V, callback: (event: V extends 'query' ? Prisma.QueryEvent : V extends 'beforeExit' ? () => Promise<void> : Prisma.LogEvent) => void): void;

  /**
   * Connect with the database
   */
  $connect(): Promise<void>;

  /**
   * Disconnect from the database
   */
  $disconnect(): Promise<void>;

  /**
   * Add a middleware
   */
  $use(cb: Prisma.Middleware): void

/**
   * Executes a prepared raw query and returns the number of affected rows.
   * @example
   * ```
   * const result = await prisma.$executeRaw`UPDATE User SET cool = ${true} WHERE email = ${'user@email.com'};`
   * ```
   * 
   * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client/raw-database-access).
   */
  $executeRaw<T = unknown>(query: TemplateStringsArray | Prisma.Sql, ...values: any[]): PrismaPromise<number>;

  /**
   * Executes a raw query and returns the number of affected rows.
   * Susceptible to SQL injections, see documentation.
   * @example
   * ```
   * const result = await prisma.$executeRawUnsafe('UPDATE User SET cool = $1 WHERE email = $2 ;', true, 'user@email.com')
   * ```
   * 
   * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client/raw-database-access).
   */
  $executeRawUnsafe<T = unknown>(query: string, ...values: any[]): PrismaPromise<number>;

  /**
   * Performs a prepared raw query and returns the `SELECT` data.
   * @example
   * ```
   * const result = await prisma.$queryRaw`SELECT * FROM User WHERE id = ${1} OR email = ${'user@email.com'};`
   * ```
   * 
   * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client/raw-database-access).
   */
  $queryRaw<T = unknown>(query: TemplateStringsArray | Prisma.Sql, ...values: any[]): PrismaPromise<T>;

  /**
   * Performs a raw query and returns the `SELECT` data.
   * Susceptible to SQL injections, see documentation.
   * @example
   * ```
   * const result = await prisma.$queryRawUnsafe('SELECT * FROM User WHERE id = $1 OR email = $2;', 1, 'user@email.com')
   * ```
   * 
   * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client/raw-database-access).
   */
  $queryRawUnsafe<T = unknown>(query: string, ...values: any[]): PrismaPromise<T>;

  /**
   * Allows the running of a sequence of read/write operations that are guaranteed to either succeed or fail as a whole.
   * @example
   * ```
   * const [george, bob, alice] = await prisma.$transaction([
   *   prisma.user.create({ data: { name: 'George' } }),
   *   prisma.user.create({ data: { name: 'Bob' } }),
   *   prisma.user.create({ data: { name: 'Alice' } }),
   * ])
   * ```
   * 
   * Read more in our [docs](https://www.prisma.io/docs/concepts/components/prisma-client/transactions).
   */
  $transaction<P extends PrismaPromise<any>[]>(arg: [...P]): Promise<UnwrapTuple<P>>;

      /**
   * `prisma.migration`: Exposes CRUD operations for the **Migration** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Migrations
    * const migrations = await prisma.migration.findMany()
    * ```
    */
  get migration(): Prisma.MigrationDelegate<GlobalReject>;

  /**
   * `prisma.library`: Exposes CRUD operations for the **Library** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Libraries
    * const libraries = await prisma.library.findMany()
    * ```
    */
  get library(): Prisma.LibraryDelegate<GlobalReject>;

  /**
   * `prisma.libraryStatistics`: Exposes CRUD operations for the **LibraryStatistics** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more LibraryStatistics
    * const libraryStatistics = await prisma.libraryStatistics.findMany()
    * ```
    */
  get libraryStatistics(): Prisma.LibraryStatisticsDelegate<GlobalReject>;

  /**
   * `prisma.client`: Exposes CRUD operations for the **Client** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Clients
    * const clients = await prisma.client.findMany()
    * ```
    */
  get client(): Prisma.ClientDelegate<GlobalReject>;

  /**
   * `prisma.location`: Exposes CRUD operations for the **Location** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Locations
    * const locations = await prisma.location.findMany()
    * ```
    */
  get location(): Prisma.LocationDelegate<GlobalReject>;

  /**
   * `prisma.file`: Exposes CRUD operations for the **File** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Files
    * const files = await prisma.file.findMany()
    * ```
    */
  get file(): Prisma.FileDelegate<GlobalReject>;

  /**
   * `prisma.tag`: Exposes CRUD operations for the **Tag** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Tags
    * const tags = await prisma.tag.findMany()
    * ```
    */
  get tag(): Prisma.TagDelegate<GlobalReject>;

  /**
   * `prisma.tagOnFile`: Exposes CRUD operations for the **TagOnFile** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more TagOnFiles
    * const tagOnFiles = await prisma.tagOnFile.findMany()
    * ```
    */
  get tagOnFile(): Prisma.TagOnFileDelegate<GlobalReject>;

  /**
   * `prisma.job`: Exposes CRUD operations for the **Job** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Jobs
    * const jobs = await prisma.job.findMany()
    * ```
    */
  get job(): Prisma.JobDelegate<GlobalReject>;

  /**
   * `prisma.space`: Exposes CRUD operations for the **Space** model.
    * Example usage:
    * ```ts
    * // Fetch zero or more Spaces
    * const spaces = await prisma.space.findMany()
    * ```
    */
  get space(): Prisma.SpaceDelegate<GlobalReject>;
}

export namespace Prisma {
  export import DMMF = runtime.DMMF

  /**
   * Prisma Errors
   */
  export import PrismaClientKnownRequestError = runtime.PrismaClientKnownRequestError
  export import PrismaClientUnknownRequestError = runtime.PrismaClientUnknownRequestError
  export import PrismaClientRustPanicError = runtime.PrismaClientRustPanicError
  export import PrismaClientInitializationError = runtime.PrismaClientInitializationError
  export import PrismaClientValidationError = runtime.PrismaClientValidationError

  /**
   * Re-export of sql-template-tag
   */
  export import sql = runtime.sqltag
  export import empty = runtime.empty
  export import join = runtime.join
  export import raw = runtime.raw
  export import Sql = runtime.Sql

  /**
   * Decimal.js
   */
  export import Decimal = runtime.Decimal

  /**
   * Prisma Client JS version: 3.10.0
   * Query Engine version: 73e60b76d394f8d37d8ebd1f8918c79029f0db86
   */
  export type PrismaVersion = {
    client: string
  }

  export const prismaVersion: PrismaVersion 

  /**
   * Utility Types
   */

  /**
   * From https://github.com/sindresorhus/type-fest/
   * Matches a JSON object.
   * This type can be useful to enforce some input to be JSON-compatible or as a super-type to be extended from. 
   */
  export type JsonObject = {[Key in string]?: JsonValue}

  /**
   * From https://github.com/sindresorhus/type-fest/
   * Matches a JSON array.
   */
  export interface JsonArray extends Array<JsonValue> {}

  /**
   * From https://github.com/sindresorhus/type-fest/
   * Matches any valid JSON value.
   */
  export type JsonValue = string | number | boolean | JsonObject | JsonArray | null

  /**
   * Matches a JSON object.
   * Unlike `JsonObject`, this type allows undefined and read-only properties.
   */
  export type InputJsonObject = {readonly [Key in string]?: InputJsonValue | null}

  /**
   * Matches a JSON array.
   * Unlike `JsonArray`, readonly arrays are assignable to this type.
   */
  export interface InputJsonArray extends ReadonlyArray<InputJsonValue | null> {}

  /**
   * Matches any valid value that can be used as an input for operations like
   * create and update as the value of a JSON field. Unlike `JsonValue`, this
   * type allows read-only arrays and read-only object properties and disallows
   * `null` at the top level.
   *
   * `null` cannot be used as the value of a JSON field because its meaning
   * would be ambiguous. Use `Prisma.JsonNull` to store the JSON null value or
   * `Prisma.DbNull` to clear the JSON value and set the field to the database
   * NULL value instead.
   *
   * @see https://www.prisma.io/docs/concepts/components/prisma-client/working-with-fields/working-with-json-fields#filtering-by-null-values
   */
  export type InputJsonValue = string | number | boolean | InputJsonObject | InputJsonArray

  /**
   * Helper for filtering JSON entries that have `null` on the database (empty on the db)
   * 
   * @see https://www.prisma.io/docs/concepts/components/prisma-client/working-with-fields/working-with-json-fields#filtering-on-a-json-field
   */
  export const DbNull: 'DbNull'

  /**
   * Helper for filtering JSON entries that have JSON `null` values (not empty on the db)
   * 
   * @see https://www.prisma.io/docs/concepts/components/prisma-client/working-with-fields/working-with-json-fields#filtering-on-a-json-field
   */
  export const JsonNull: 'JsonNull'

  /**
   * Helper for filtering JSON entries that are `Prisma.DbNull` or `Prisma.JsonNull`
   * 
   * @see https://www.prisma.io/docs/concepts/components/prisma-client/working-with-fields/working-with-json-fields#filtering-on-a-json-field
   */
  export const AnyNull: 'AnyNull'

  type SelectAndInclude = {
    select: any
    include: any
  }
  type HasSelect = {
    select: any
  }
  type HasInclude = {
    include: any
  }
  type CheckSelect<T, S, U> = T extends SelectAndInclude
    ? 'Please either choose `select` or `include`'
    : T extends HasSelect
    ? U
    : T extends HasInclude
    ? U
    : S

  /**
   * Get the type of the value, that the Promise holds.
   */
  export type PromiseType<T extends PromiseLike<any>> = T extends PromiseLike<infer U> ? U : T;

  /**
   * Get the return type of a function which returns a Promise.
   */
  export type PromiseReturnType<T extends (...args: any) => Promise<any>> = PromiseType<ReturnType<T>>

  /**
   * From T, pick a set of properties whose keys are in the union K
   */
  type Prisma__Pick<T, K extends keyof T> = {
      [P in K]: T[P];
  };


  export type Enumerable<T> = T | Array<T>;

  export type RequiredKeys<T> = {
    [K in keyof T]-?: {} extends Prisma__Pick<T, K> ? never : K
  }[keyof T]

  export type TruthyKeys<T> = {
    [key in keyof T]: T[key] extends false | undefined | null ? never : key
  }[keyof T]

  export type TrueKeys<T> = TruthyKeys<Prisma__Pick<T, RequiredKeys<T>>>

  /**
   * Subset
   * @desc From `T` pick properties that exist in `U`. Simple version of Intersection
   */
  export type Subset<T, U> = {
    [key in keyof T]: key extends keyof U ? T[key] : never;
  };

  /**
   * SelectSubset
   * @desc From `T` pick properties that exist in `U`. Simple version of Intersection.
   * Additionally, it validates, if both select and include are present. If the case, it errors.
   */
  export type SelectSubset<T, U> = {
    [key in keyof T]: key extends keyof U ? T[key] : never
  } &
    (T extends SelectAndInclude
      ? 'Please either choose `select` or `include`.'
      : {})

  /**
   * Subset + Intersection
   * @desc From `T` pick properties that exist in `U` and intersect `K`
   */
  export type SubsetIntersection<T, U, K> = {
    [key in keyof T]: key extends keyof U ? T[key] : never
  } &
    K

  type Without<T, U> = { [P in Exclude<keyof T, keyof U>]?: never };

  /**
   * XOR is needed to have a real mutually exclusive union type
   * https://stackoverflow.com/questions/42123407/does-typescript-support-mutually-exclusive-types
   */
  type XOR<T, U> =
    T extends object ?
    U extends object ?
      (Without<T, U> & U) | (Without<U, T> & T)
    : U : T


  /**
   * Is T a Record?
   */
  type IsObject<T extends any> = T extends Array<any>
  ? False
  : T extends Date
  ? False
  : T extends Buffer
  ? False
  : T extends BigInt
  ? False
  : T extends object
  ? True
  : False


  /**
   * If it's T[], return T
   */
  export type UnEnumerate<T extends unknown> = T extends Array<infer U> ? U : T

  /**
   * From ts-toolbelt
   */

  type __Either<O extends object, K extends Key> = Omit<O, K> &
    {
      // Merge all but K
      [P in K]: Prisma__Pick<O, P & keyof O> // With K possibilities
    }[K]

  type EitherStrict<O extends object, K extends Key> = Strict<__Either<O, K>>

  type EitherLoose<O extends object, K extends Key> = ComputeRaw<__Either<O, K>>

  type _Either<
    O extends object,
    K extends Key,
    strict extends Boolean
  > = {
    1: EitherStrict<O, K>
    0: EitherLoose<O, K>
  }[strict]

  type Either<
    O extends object,
    K extends Key,
    strict extends Boolean = 1
  > = O extends unknown ? _Either<O, K, strict> : never

  export type Union = any

  type PatchUndefined<O extends object, O1 extends object> = {
    [K in keyof O]: O[K] extends undefined ? At<O1, K> : O[K]
  } & {}

  /** Helper Types for "Merge" **/
  export type IntersectOf<U extends Union> = (
    U extends unknown ? (k: U) => void : never
  ) extends (k: infer I) => void
    ? I
    : never

  export type Overwrite<O extends object, O1 extends object> = {
      [K in keyof O]: K extends keyof O1 ? O1[K] : O[K];
  } & {};

  type _Merge<U extends object> = IntersectOf<Overwrite<U, {
      [K in keyof U]-?: At<U, K>;
  }>>;

  type Key = string | number | symbol;
  type AtBasic<O extends object, K extends Key> = K extends keyof O ? O[K] : never;
  type AtStrict<O extends object, K extends Key> = O[K & keyof O];
  type AtLoose<O extends object, K extends Key> = O extends unknown ? AtStrict<O, K> : never;
  export type At<O extends object, K extends Key, strict extends Boolean = 1> = {
      1: AtStrict<O, K>;
      0: AtLoose<O, K>;
  }[strict];

  export type ComputeRaw<A extends any> = A extends Function ? A : {
    [K in keyof A]: A[K];
  } & {};

  export type OptionalFlat<O> = {
    [K in keyof O]?: O[K];
  } & {};

  type _Record<K extends keyof any, T> = {
    [P in K]: T;
  };

  type _Strict<U, _U = U> = U extends unknown ? U & OptionalFlat<_Record<Exclude<Keys<_U>, keyof U>, never>> : never;

  export type Strict<U extends object> = ComputeRaw<_Strict<U>>;
  /** End Helper Types for "Merge" **/

  export type Merge<U extends object> = ComputeRaw<_Merge<Strict<U>>>;

  /**
  A [[Boolean]]
  */
  export type Boolean = True | False

  // /**
  // 1
  // */
  export type True = 1

  /**
  0
  */
  export type False = 0

  export type Not<B extends Boolean> = {
    0: 1
    1: 0
  }[B]

  export type Extends<A1 extends any, A2 extends any> = [A1] extends [never]
    ? 0 // anything `never` is false
    : A1 extends A2
    ? 1
    : 0

  export type Has<U extends Union, U1 extends Union> = Not<
    Extends<Exclude<U1, U>, U1>
  >

  export type Or<B1 extends Boolean, B2 extends Boolean> = {
    0: {
      0: 0
      1: 1
    }
    1: {
      0: 1
      1: 1
    }
  }[B1][B2]

  export type Keys<U extends Union> = U extends unknown ? keyof U : never

  type Exact<A, W = unknown> = 
  W extends unknown ? A extends Narrowable ? Cast<A, W> : Cast<
  {[K in keyof A]: K extends keyof W ? Exact<A[K], W[K]> : never},
  {[K in keyof W]: K extends keyof A ? Exact<A[K], W[K]> : W[K]}>
  : never;

  type Narrowable = string | number | boolean | bigint;

  type Cast<A, B> = A extends B ? A : B;

  export const type: unique symbol;

  export function validator<V>(): <S>(select: Exact<S, V>) => S;

  /**
   * Used by group by
   */

  export type GetScalarType<T, O> = O extends object ? {
    [P in keyof T]: P extends keyof O
      ? O[P]
      : never
  } : never

  type FieldPaths<
    T,
    U = Omit<T, '_avg' | '_sum' | '_count' | '_min' | '_max'>
  > = IsObject<T> extends True ? U : T

  type GetHavingFields<T> = {
    [K in keyof T]: Or<
      Or<Extends<'OR', K>, Extends<'AND', K>>,
      Extends<'NOT', K>
    > extends True
      ? // infer is only needed to not hit TS limit
        // based on the brilliant idea of Pierre-Antoine Mills
        // https://github.com/microsoft/TypeScript/issues/30188#issuecomment-478938437
        T[K] extends infer TK
        ? GetHavingFields<UnEnumerate<TK> extends object ? Merge<UnEnumerate<TK>> : never>
        : never
      : {} extends FieldPaths<T[K]>
      ? never
      : K
  }[keyof T]

  /**
   * Convert tuple to union
   */
  type _TupleToUnion<T> = T extends (infer E)[] ? E : never
  type TupleToUnion<K extends readonly any[]> = _TupleToUnion<K>
  type MaybeTupleToUnion<T> = T extends any[] ? TupleToUnion<T> : T

  /**
   * Like `Pick`, but with an array
   */
  type PickArray<T, K extends Array<keyof T>> = Prisma__Pick<T, TupleToUnion<K>>

  /**
   * Exclude all keys with underscores
   */
  type ExcludeUnderscoreKeys<T extends string> = T extends `_${string}` ? never : T

  class PrismaClientFetcher {
    private readonly prisma;
    private readonly debug;
    private readonly hooks?;
    constructor(prisma: PrismaClient<any, any>, debug?: boolean, hooks?: Hooks | undefined);
    request<T>(document: any, dataPath?: string[], rootField?: string, typeName?: string, isList?: boolean, callsite?: string): Promise<T>;
    sanitizeMessage(message: string): string;
    protected unpack(document: any, data: any, path: string[], rootField?: string, isList?: boolean): any;
  }

  export const ModelName: {
    Migration: 'Migration',
    Library: 'Library',
    LibraryStatistics: 'LibraryStatistics',
    Client: 'Client',
    Location: 'Location',
    File: 'File',
    Tag: 'Tag',
    TagOnFile: 'TagOnFile',
    Job: 'Job',
    Space: 'Space'
  };

  export type ModelName = (typeof ModelName)[keyof typeof ModelName]


  export type Datasources = {
    db?: Datasource
  }

  export type RejectOnNotFound = boolean | ((error: Error) => Error)
  export type RejectPerModel = { [P in ModelName]?: RejectOnNotFound }
  export type RejectPerOperation =  { [P in "findUnique" | "findFirst"]?: RejectPerModel | RejectOnNotFound } 
  type IsReject<T> = T extends true ? True : T extends (err: Error) => Error ? True : False
  export type HasReject<
    GlobalRejectSettings extends Prisma.PrismaClientOptions['rejectOnNotFound'],
    LocalRejectSettings,
    Action extends PrismaAction,
    Model extends ModelName
  > = LocalRejectSettings extends RejectOnNotFound
    ? IsReject<LocalRejectSettings>
    : GlobalRejectSettings extends RejectPerOperation
    ? Action extends keyof GlobalRejectSettings
      ? GlobalRejectSettings[Action] extends boolean
        ? IsReject<GlobalRejectSettings[Action]>
        : GlobalRejectSettings[Action] extends RejectPerModel
        ? Model extends keyof GlobalRejectSettings[Action]
          ? IsReject<GlobalRejectSettings[Action][Model]>
          : False
        : False
      : False
    : IsReject<GlobalRejectSettings>
  export type ErrorFormat = 'pretty' | 'colorless' | 'minimal'

  export interface PrismaClientOptions {
    /**
     * Configure findUnique/findFirst to throw an error if the query returns null. 
     *  * @example
     * ```
     * // Reject on both findUnique/findFirst
     * rejectOnNotFound: true
     * // Reject only on findFirst with a custom error
     * rejectOnNotFound: { findFirst: (err) => new Error("Custom Error")}
     * // Reject on user.findUnique with a custom error
     * rejectOnNotFound: { findUnique: {User: (err) => new Error("User not found")}}
     * ```
     */
    rejectOnNotFound?: RejectOnNotFound | RejectPerOperation
    /**
     * Overwrites the datasource url from your prisma.schema file
     */
    datasources?: Datasources

    /**
     * @default "colorless"
     */
    errorFormat?: ErrorFormat

    /**
     * @example
     * ```
     * // Defaults to stdout
     * log: ['query', 'info', 'warn', 'error']
     * 
     * // Emit as events
     * log: [
     *  { emit: 'stdout', level: 'query' },
     *  { emit: 'stdout', level: 'info' },
     *  { emit: 'stdout', level: 'warn' }
     *  { emit: 'stdout', level: 'error' }
     * ]
     * ```
     * Read more in our [docs](https://www.prisma.io/docs/reference/tools-and-interfaces/prisma-client/logging#the-log-option).
     */
    log?: Array<LogLevel | LogDefinition>
  }

  export type Hooks = {
    beforeRequest?: (options: { query: string, path: string[], rootField?: string, typeName?: string, document: any }) => any
  }

  /* Types for Logging */
  export type LogLevel = 'info' | 'query' | 'warn' | 'error'
  export type LogDefinition = {
    level: LogLevel
    emit: 'stdout' | 'event'
  }

  export type GetLogType<T extends LogLevel | LogDefinition> = T extends LogDefinition ? T['emit'] extends 'event' ? T['level'] : never : never
  export type GetEvents<T extends any> = T extends Array<LogLevel | LogDefinition> ?
    GetLogType<T[0]> | GetLogType<T[1]> | GetLogType<T[2]> | GetLogType<T[3]>
    : never

  export type QueryEvent = {
    timestamp: Date
    query: string
    params: string
    duration: number
    target: string
  }

  export type LogEvent = {
    timestamp: Date
    message: string
    target: string
  }
  /* End Types for Logging */


  export type PrismaAction =
    | 'findUnique'
    | 'findMany'
    | 'findFirst'
    | 'create'
    | 'createMany'
    | 'update'
    | 'updateMany'
    | 'upsert'
    | 'delete'
    | 'deleteMany'
    | 'executeRaw'
    | 'queryRaw'
    | 'aggregate'
    | 'count'
    | 'runCommandRaw'

  /**
   * These options are being passed in to the middleware as "params"
   */
  export type MiddlewareParams = {
    model?: ModelName
    action: PrismaAction
    args: any
    dataPath: string[]
    runInTransaction: boolean
  }

  /**
   * The `T` type makes sure, that the `return proceed` is not forgotten in the middleware implementation
   */
  export type Middleware<T = any> = (
    params: MiddlewareParams,
    next: (params: MiddlewareParams) => Promise<T>,
  ) => Promise<T>

  // tested in getLogLevel.test.ts
  export function getLogLevel(log: Array<LogLevel | LogDefinition>): LogLevel | undefined; 
  export type Datasource = {
    url?: string
  }

  /**
   * Count Types
   */


  /**
   * Count Type LibraryCountOutputType
   */


  export type LibraryCountOutputType = {
    spaces: number
  }

  export type LibraryCountOutputTypeSelect = {
    spaces?: boolean
  }

  export type LibraryCountOutputTypeGetPayload<
    S extends boolean | null | undefined | LibraryCountOutputTypeArgs,
    U = keyof S
      > = S extends true
        ? LibraryCountOutputType
    : S extends undefined
    ? never
    : S extends LibraryCountOutputTypeArgs
    ?'include' extends U
    ? LibraryCountOutputType 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof LibraryCountOutputType ? LibraryCountOutputType[P] : never
  } 
    : LibraryCountOutputType
  : LibraryCountOutputType




  // Custom InputTypes

  /**
   * LibraryCountOutputType without action
   */
  export type LibraryCountOutputTypeArgs = {
    /**
     * Select specific fields to fetch from the LibraryCountOutputType
     * 
    **/
    select?: LibraryCountOutputTypeSelect | null
  }



  /**
   * Count Type ClientCountOutputType
   */


  export type ClientCountOutputType = {
    jobs: number
  }

  export type ClientCountOutputTypeSelect = {
    jobs?: boolean
  }

  export type ClientCountOutputTypeGetPayload<
    S extends boolean | null | undefined | ClientCountOutputTypeArgs,
    U = keyof S
      > = S extends true
        ? ClientCountOutputType
    : S extends undefined
    ? never
    : S extends ClientCountOutputTypeArgs
    ?'include' extends U
    ? ClientCountOutputType 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof ClientCountOutputType ? ClientCountOutputType[P] : never
  } 
    : ClientCountOutputType
  : ClientCountOutputType




  // Custom InputTypes

  /**
   * ClientCountOutputType without action
   */
  export type ClientCountOutputTypeArgs = {
    /**
     * Select specific fields to fetch from the ClientCountOutputType
     * 
    **/
    select?: ClientCountOutputTypeSelect | null
  }



  /**
   * Count Type LocationCountOutputType
   */


  export type LocationCountOutputType = {
    files: number
  }

  export type LocationCountOutputTypeSelect = {
    files?: boolean
  }

  export type LocationCountOutputTypeGetPayload<
    S extends boolean | null | undefined | LocationCountOutputTypeArgs,
    U = keyof S
      > = S extends true
        ? LocationCountOutputType
    : S extends undefined
    ? never
    : S extends LocationCountOutputTypeArgs
    ?'include' extends U
    ? LocationCountOutputType 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof LocationCountOutputType ? LocationCountOutputType[P] : never
  } 
    : LocationCountOutputType
  : LocationCountOutputType




  // Custom InputTypes

  /**
   * LocationCountOutputType without action
   */
  export type LocationCountOutputTypeArgs = {
    /**
     * Select specific fields to fetch from the LocationCountOutputType
     * 
    **/
    select?: LocationCountOutputTypeSelect | null
  }



  /**
   * Count Type FileCountOutputType
   */


  export type FileCountOutputType = {
    children: number
    file_tags: number
  }

  export type FileCountOutputTypeSelect = {
    children?: boolean
    file_tags?: boolean
  }

  export type FileCountOutputTypeGetPayload<
    S extends boolean | null | undefined | FileCountOutputTypeArgs,
    U = keyof S
      > = S extends true
        ? FileCountOutputType
    : S extends undefined
    ? never
    : S extends FileCountOutputTypeArgs
    ?'include' extends U
    ? FileCountOutputType 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof FileCountOutputType ? FileCountOutputType[P] : never
  } 
    : FileCountOutputType
  : FileCountOutputType




  // Custom InputTypes

  /**
   * FileCountOutputType without action
   */
  export type FileCountOutputTypeArgs = {
    /**
     * Select specific fields to fetch from the FileCountOutputType
     * 
    **/
    select?: FileCountOutputTypeSelect | null
  }



  /**
   * Count Type TagCountOutputType
   */


  export type TagCountOutputType = {
    tag_files: number
  }

  export type TagCountOutputTypeSelect = {
    tag_files?: boolean
  }

  export type TagCountOutputTypeGetPayload<
    S extends boolean | null | undefined | TagCountOutputTypeArgs,
    U = keyof S
      > = S extends true
        ? TagCountOutputType
    : S extends undefined
    ? never
    : S extends TagCountOutputTypeArgs
    ?'include' extends U
    ? TagCountOutputType 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof TagCountOutputType ? TagCountOutputType[P] : never
  } 
    : TagCountOutputType
  : TagCountOutputType




  // Custom InputTypes

  /**
   * TagCountOutputType without action
   */
  export type TagCountOutputTypeArgs = {
    /**
     * Select specific fields to fetch from the TagCountOutputType
     * 
    **/
    select?: TagCountOutputTypeSelect | null
  }



  /**
   * Models
   */

  /**
   * Model Migration
   */


  export type AggregateMigration = {
    _count: MigrationCountAggregateOutputType | null
    _avg: MigrationAvgAggregateOutputType | null
    _sum: MigrationSumAggregateOutputType | null
    _min: MigrationMinAggregateOutputType | null
    _max: MigrationMaxAggregateOutputType | null
  }

  export type MigrationAvgAggregateOutputType = {
    id: number | null
    steps_applied: number | null
  }

  export type MigrationSumAggregateOutputType = {
    id: number | null
    steps_applied: number | null
  }

  export type MigrationMinAggregateOutputType = {
    id: number | null
    name: string | null
    checksum: string | null
    steps_applied: number | null
    applied_at: Date | null
  }

  export type MigrationMaxAggregateOutputType = {
    id: number | null
    name: string | null
    checksum: string | null
    steps_applied: number | null
    applied_at: Date | null
  }

  export type MigrationCountAggregateOutputType = {
    id: number
    name: number
    checksum: number
    steps_applied: number
    applied_at: number
    _all: number
  }


  export type MigrationAvgAggregateInputType = {
    id?: true
    steps_applied?: true
  }

  export type MigrationSumAggregateInputType = {
    id?: true
    steps_applied?: true
  }

  export type MigrationMinAggregateInputType = {
    id?: true
    name?: true
    checksum?: true
    steps_applied?: true
    applied_at?: true
  }

  export type MigrationMaxAggregateInputType = {
    id?: true
    name?: true
    checksum?: true
    steps_applied?: true
    applied_at?: true
  }

  export type MigrationCountAggregateInputType = {
    id?: true
    name?: true
    checksum?: true
    steps_applied?: true
    applied_at?: true
    _all?: true
  }

  export type MigrationAggregateArgs = {
    /**
     * Filter which Migration to aggregate.
     * 
    **/
    where?: MigrationWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Migrations to fetch.
     * 
    **/
    orderBy?: Enumerable<MigrationOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: MigrationWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Migrations from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Migrations.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Migrations
    **/
    _count?: true | MigrationCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: MigrationAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: MigrationSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: MigrationMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: MigrationMaxAggregateInputType
  }

  export type GetMigrationAggregateType<T extends MigrationAggregateArgs> = {
        [P in keyof T & keyof AggregateMigration]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateMigration[P]>
      : GetScalarType<T[P], AggregateMigration[P]>
  }




  export type MigrationGroupByArgs = {
    where?: MigrationWhereInput
    orderBy?: Enumerable<MigrationOrderByWithAggregationInput>
    by: Array<MigrationScalarFieldEnum>
    having?: MigrationScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: MigrationCountAggregateInputType | true
    _avg?: MigrationAvgAggregateInputType
    _sum?: MigrationSumAggregateInputType
    _min?: MigrationMinAggregateInputType
    _max?: MigrationMaxAggregateInputType
  }


  export type MigrationGroupByOutputType = {
    id: number
    name: string
    checksum: string
    steps_applied: number
    applied_at: Date
    _count: MigrationCountAggregateOutputType | null
    _avg: MigrationAvgAggregateOutputType | null
    _sum: MigrationSumAggregateOutputType | null
    _min: MigrationMinAggregateOutputType | null
    _max: MigrationMaxAggregateOutputType | null
  }

  type GetMigrationGroupByPayload<T extends MigrationGroupByArgs> = PrismaPromise<
    Array<
      PickArray<MigrationGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof MigrationGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], MigrationGroupByOutputType[P]>
            : GetScalarType<T[P], MigrationGroupByOutputType[P]>
        }
      >
    >


  export type MigrationSelect = {
    id?: boolean
    name?: boolean
    checksum?: boolean
    steps_applied?: boolean
    applied_at?: boolean
  }

  export type MigrationGetPayload<
    S extends boolean | null | undefined | MigrationArgs,
    U = keyof S
      > = S extends true
        ? Migration
    : S extends undefined
    ? never
    : S extends MigrationArgs | MigrationFindManyArgs
    ?'include' extends U
    ? Migration 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof Migration ? Migration[P] : never
  } 
    : Migration
  : Migration


  type MigrationCountArgs = Merge<
    Omit<MigrationFindManyArgs, 'select' | 'include'> & {
      select?: MigrationCountAggregateInputType | true
    }
  >

  export interface MigrationDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Migration that matches the filter.
     * @param {MigrationFindUniqueArgs} args - Arguments to find a Migration
     * @example
     * // Get one Migration
     * const migration = await prisma.migration.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends MigrationFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, MigrationFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Migration'> extends True ? CheckSelect<T, Prisma__MigrationClient<Migration>, Prisma__MigrationClient<MigrationGetPayload<T>>> : CheckSelect<T, Prisma__MigrationClient<Migration | null >, Prisma__MigrationClient<MigrationGetPayload<T> | null >>

    /**
     * Find the first Migration that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {MigrationFindFirstArgs} args - Arguments to find a Migration
     * @example
     * // Get one Migration
     * const migration = await prisma.migration.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends MigrationFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, MigrationFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Migration'> extends True ? CheckSelect<T, Prisma__MigrationClient<Migration>, Prisma__MigrationClient<MigrationGetPayload<T>>> : CheckSelect<T, Prisma__MigrationClient<Migration | null >, Prisma__MigrationClient<MigrationGetPayload<T> | null >>

    /**
     * Find zero or more Migrations that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {MigrationFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Migrations
     * const migrations = await prisma.migration.findMany()
     * 
     * // Get first 10 Migrations
     * const migrations = await prisma.migration.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const migrationWithIdOnly = await prisma.migration.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends MigrationFindManyArgs>(
      args?: SelectSubset<T, MigrationFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Migration>>, PrismaPromise<Array<MigrationGetPayload<T>>>>

    /**
     * Create a Migration.
     * @param {MigrationCreateArgs} args - Arguments to create a Migration.
     * @example
     * // Create one Migration
     * const Migration = await prisma.migration.create({
     *   data: {
     *     // ... data to create a Migration
     *   }
     * })
     * 
    **/
    create<T extends MigrationCreateArgs>(
      args: SelectSubset<T, MigrationCreateArgs>
    ): CheckSelect<T, Prisma__MigrationClient<Migration>, Prisma__MigrationClient<MigrationGetPayload<T>>>

    /**
     * Delete a Migration.
     * @param {MigrationDeleteArgs} args - Arguments to delete one Migration.
     * @example
     * // Delete one Migration
     * const Migration = await prisma.migration.delete({
     *   where: {
     *     // ... filter to delete one Migration
     *   }
     * })
     * 
    **/
    delete<T extends MigrationDeleteArgs>(
      args: SelectSubset<T, MigrationDeleteArgs>
    ): CheckSelect<T, Prisma__MigrationClient<Migration>, Prisma__MigrationClient<MigrationGetPayload<T>>>

    /**
     * Update one Migration.
     * @param {MigrationUpdateArgs} args - Arguments to update one Migration.
     * @example
     * // Update one Migration
     * const migration = await prisma.migration.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends MigrationUpdateArgs>(
      args: SelectSubset<T, MigrationUpdateArgs>
    ): CheckSelect<T, Prisma__MigrationClient<Migration>, Prisma__MigrationClient<MigrationGetPayload<T>>>

    /**
     * Delete zero or more Migrations.
     * @param {MigrationDeleteManyArgs} args - Arguments to filter Migrations to delete.
     * @example
     * // Delete a few Migrations
     * const { count } = await prisma.migration.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends MigrationDeleteManyArgs>(
      args?: SelectSubset<T, MigrationDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Migrations.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {MigrationUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Migrations
     * const migration = await prisma.migration.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends MigrationUpdateManyArgs>(
      args: SelectSubset<T, MigrationUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Migration.
     * @param {MigrationUpsertArgs} args - Arguments to update or create a Migration.
     * @example
     * // Update or create a Migration
     * const migration = await prisma.migration.upsert({
     *   create: {
     *     // ... data to create a Migration
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Migration we want to update
     *   }
     * })
    **/
    upsert<T extends MigrationUpsertArgs>(
      args: SelectSubset<T, MigrationUpsertArgs>
    ): CheckSelect<T, Prisma__MigrationClient<Migration>, Prisma__MigrationClient<MigrationGetPayload<T>>>

    /**
     * Count the number of Migrations.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {MigrationCountArgs} args - Arguments to filter Migrations to count.
     * @example
     * // Count the number of Migrations
     * const count = await prisma.migration.count({
     *   where: {
     *     // ... the filter for the Migrations we want to count
     *   }
     * })
    **/
    count<T extends MigrationCountArgs>(
      args?: Subset<T, MigrationCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], MigrationCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Migration.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {MigrationAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends MigrationAggregateArgs>(args: Subset<T, MigrationAggregateArgs>): PrismaPromise<GetMigrationAggregateType<T>>

    /**
     * Group by Migration.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {MigrationGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends MigrationGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: MigrationGroupByArgs['orderBy'] }
        : { orderBy?: MigrationGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, MigrationGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetMigrationGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Migration.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__MigrationClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';


    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Migration findUnique
   */
  export type MigrationFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * Throw an Error if a Migration can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Migration to fetch.
     * 
    **/
    where: MigrationWhereUniqueInput
  }


  /**
   * Migration findFirst
   */
  export type MigrationFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * Throw an Error if a Migration can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Migration to fetch.
     * 
    **/
    where?: MigrationWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Migrations to fetch.
     * 
    **/
    orderBy?: Enumerable<MigrationOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Migrations.
     * 
    **/
    cursor?: MigrationWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Migrations from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Migrations.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Migrations.
     * 
    **/
    distinct?: Enumerable<MigrationScalarFieldEnum>
  }


  /**
   * Migration findMany
   */
  export type MigrationFindManyArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * Filter, which Migrations to fetch.
     * 
    **/
    where?: MigrationWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Migrations to fetch.
     * 
    **/
    orderBy?: Enumerable<MigrationOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Migrations.
     * 
    **/
    cursor?: MigrationWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Migrations from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Migrations.
     * 
    **/
    skip?: number
    distinct?: Enumerable<MigrationScalarFieldEnum>
  }


  /**
   * Migration create
   */
  export type MigrationCreateArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * The data needed to create a Migration.
     * 
    **/
    data: XOR<MigrationCreateInput, MigrationUncheckedCreateInput>
  }


  /**
   * Migration update
   */
  export type MigrationUpdateArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * The data needed to update a Migration.
     * 
    **/
    data: XOR<MigrationUpdateInput, MigrationUncheckedUpdateInput>
    /**
     * Choose, which Migration to update.
     * 
    **/
    where: MigrationWhereUniqueInput
  }


  /**
   * Migration updateMany
   */
  export type MigrationUpdateManyArgs = {
    /**
     * The data used to update Migrations.
     * 
    **/
    data: XOR<MigrationUpdateManyMutationInput, MigrationUncheckedUpdateManyInput>
    /**
     * Filter which Migrations to update
     * 
    **/
    where?: MigrationWhereInput
  }


  /**
   * Migration upsert
   */
  export type MigrationUpsertArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * The filter to search for the Migration to update in case it exists.
     * 
    **/
    where: MigrationWhereUniqueInput
    /**
     * In case the Migration found by the `where` argument doesn't exist, create a new Migration with this data.
     * 
    **/
    create: XOR<MigrationCreateInput, MigrationUncheckedCreateInput>
    /**
     * In case the Migration was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<MigrationUpdateInput, MigrationUncheckedUpdateInput>
  }


  /**
   * Migration delete
   */
  export type MigrationDeleteArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
    /**
     * Filter which Migration to delete.
     * 
    **/
    where: MigrationWhereUniqueInput
  }


  /**
   * Migration deleteMany
   */
  export type MigrationDeleteManyArgs = {
    /**
     * Filter which Migrations to delete
     * 
    **/
    where?: MigrationWhereInput
  }


  /**
   * Migration without action
   */
  export type MigrationArgs = {
    /**
     * Select specific fields to fetch from the Migration
     * 
    **/
    select?: MigrationSelect | null
  }



  /**
   * Model Library
   */


  export type AggregateLibrary = {
    _count: LibraryCountAggregateOutputType | null
    _avg: LibraryAvgAggregateOutputType | null
    _sum: LibrarySumAggregateOutputType | null
    _min: LibraryMinAggregateOutputType | null
    _max: LibraryMaxAggregateOutputType | null
  }

  export type LibraryAvgAggregateOutputType = {
    id: number | null
    encryption: number | null
  }

  export type LibrarySumAggregateOutputType = {
    id: number | null
    encryption: number | null
  }

  export type LibraryMinAggregateOutputType = {
    id: number | null
    uuid: string | null
    name: string | null
    remote_id: string | null
    is_primary: boolean | null
    encryption: number | null
    date_created: Date | null
    timezone: string | null
  }

  export type LibraryMaxAggregateOutputType = {
    id: number | null
    uuid: string | null
    name: string | null
    remote_id: string | null
    is_primary: boolean | null
    encryption: number | null
    date_created: Date | null
    timezone: string | null
  }

  export type LibraryCountAggregateOutputType = {
    id: number
    uuid: number
    name: number
    remote_id: number
    is_primary: number
    encryption: number
    date_created: number
    timezone: number
    _all: number
  }


  export type LibraryAvgAggregateInputType = {
    id?: true
    encryption?: true
  }

  export type LibrarySumAggregateInputType = {
    id?: true
    encryption?: true
  }

  export type LibraryMinAggregateInputType = {
    id?: true
    uuid?: true
    name?: true
    remote_id?: true
    is_primary?: true
    encryption?: true
    date_created?: true
    timezone?: true
  }

  export type LibraryMaxAggregateInputType = {
    id?: true
    uuid?: true
    name?: true
    remote_id?: true
    is_primary?: true
    encryption?: true
    date_created?: true
    timezone?: true
  }

  export type LibraryCountAggregateInputType = {
    id?: true
    uuid?: true
    name?: true
    remote_id?: true
    is_primary?: true
    encryption?: true
    date_created?: true
    timezone?: true
    _all?: true
  }

  export type LibraryAggregateArgs = {
    /**
     * Filter which Library to aggregate.
     * 
    **/
    where?: LibraryWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Libraries to fetch.
     * 
    **/
    orderBy?: Enumerable<LibraryOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: LibraryWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Libraries from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Libraries.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Libraries
    **/
    _count?: true | LibraryCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: LibraryAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: LibrarySumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: LibraryMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: LibraryMaxAggregateInputType
  }

  export type GetLibraryAggregateType<T extends LibraryAggregateArgs> = {
        [P in keyof T & keyof AggregateLibrary]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateLibrary[P]>
      : GetScalarType<T[P], AggregateLibrary[P]>
  }




  export type LibraryGroupByArgs = {
    where?: LibraryWhereInput
    orderBy?: Enumerable<LibraryOrderByWithAggregationInput>
    by: Array<LibraryScalarFieldEnum>
    having?: LibraryScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: LibraryCountAggregateInputType | true
    _avg?: LibraryAvgAggregateInputType
    _sum?: LibrarySumAggregateInputType
    _min?: LibraryMinAggregateInputType
    _max?: LibraryMaxAggregateInputType
  }


  export type LibraryGroupByOutputType = {
    id: number
    uuid: string
    name: string
    remote_id: string | null
    is_primary: boolean
    encryption: number
    date_created: Date
    timezone: string | null
    _count: LibraryCountAggregateOutputType | null
    _avg: LibraryAvgAggregateOutputType | null
    _sum: LibrarySumAggregateOutputType | null
    _min: LibraryMinAggregateOutputType | null
    _max: LibraryMaxAggregateOutputType | null
  }

  type GetLibraryGroupByPayload<T extends LibraryGroupByArgs> = PrismaPromise<
    Array<
      PickArray<LibraryGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof LibraryGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], LibraryGroupByOutputType[P]>
            : GetScalarType<T[P], LibraryGroupByOutputType[P]>
        }
      >
    >


  export type LibrarySelect = {
    id?: boolean
    uuid?: boolean
    name?: boolean
    remote_id?: boolean
    is_primary?: boolean
    encryption?: boolean
    date_created?: boolean
    timezone?: boolean
    spaces?: boolean | SpaceFindManyArgs
    _count?: boolean | LibraryCountOutputTypeArgs
  }

  export type LibraryInclude = {
    spaces?: boolean | SpaceFindManyArgs
    _count?: boolean | LibraryCountOutputTypeArgs
  }

  export type LibraryGetPayload<
    S extends boolean | null | undefined | LibraryArgs,
    U = keyof S
      > = S extends true
        ? Library
    : S extends undefined
    ? never
    : S extends LibraryArgs | LibraryFindManyArgs
    ?'include' extends U
    ? Library  & {
    [P in TrueKeys<S['include']>]:
        P extends 'spaces' ? Array < SpaceGetPayload<S['include'][P]>>  :
        P extends '_count' ? LibraryCountOutputTypeGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'spaces' ? Array < SpaceGetPayload<S['select'][P]>>  :
        P extends '_count' ? LibraryCountOutputTypeGetPayload<S['select'][P]> :  P extends keyof Library ? Library[P] : never
  } 
    : Library
  : Library


  type LibraryCountArgs = Merge<
    Omit<LibraryFindManyArgs, 'select' | 'include'> & {
      select?: LibraryCountAggregateInputType | true
    }
  >

  export interface LibraryDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Library that matches the filter.
     * @param {LibraryFindUniqueArgs} args - Arguments to find a Library
     * @example
     * // Get one Library
     * const library = await prisma.library.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends LibraryFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, LibraryFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Library'> extends True ? CheckSelect<T, Prisma__LibraryClient<Library>, Prisma__LibraryClient<LibraryGetPayload<T>>> : CheckSelect<T, Prisma__LibraryClient<Library | null >, Prisma__LibraryClient<LibraryGetPayload<T> | null >>

    /**
     * Find the first Library that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryFindFirstArgs} args - Arguments to find a Library
     * @example
     * // Get one Library
     * const library = await prisma.library.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends LibraryFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, LibraryFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Library'> extends True ? CheckSelect<T, Prisma__LibraryClient<Library>, Prisma__LibraryClient<LibraryGetPayload<T>>> : CheckSelect<T, Prisma__LibraryClient<Library | null >, Prisma__LibraryClient<LibraryGetPayload<T> | null >>

    /**
     * Find zero or more Libraries that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Libraries
     * const libraries = await prisma.library.findMany()
     * 
     * // Get first 10 Libraries
     * const libraries = await prisma.library.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const libraryWithIdOnly = await prisma.library.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends LibraryFindManyArgs>(
      args?: SelectSubset<T, LibraryFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Library>>, PrismaPromise<Array<LibraryGetPayload<T>>>>

    /**
     * Create a Library.
     * @param {LibraryCreateArgs} args - Arguments to create a Library.
     * @example
     * // Create one Library
     * const Library = await prisma.library.create({
     *   data: {
     *     // ... data to create a Library
     *   }
     * })
     * 
    **/
    create<T extends LibraryCreateArgs>(
      args: SelectSubset<T, LibraryCreateArgs>
    ): CheckSelect<T, Prisma__LibraryClient<Library>, Prisma__LibraryClient<LibraryGetPayload<T>>>

    /**
     * Delete a Library.
     * @param {LibraryDeleteArgs} args - Arguments to delete one Library.
     * @example
     * // Delete one Library
     * const Library = await prisma.library.delete({
     *   where: {
     *     // ... filter to delete one Library
     *   }
     * })
     * 
    **/
    delete<T extends LibraryDeleteArgs>(
      args: SelectSubset<T, LibraryDeleteArgs>
    ): CheckSelect<T, Prisma__LibraryClient<Library>, Prisma__LibraryClient<LibraryGetPayload<T>>>

    /**
     * Update one Library.
     * @param {LibraryUpdateArgs} args - Arguments to update one Library.
     * @example
     * // Update one Library
     * const library = await prisma.library.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends LibraryUpdateArgs>(
      args: SelectSubset<T, LibraryUpdateArgs>
    ): CheckSelect<T, Prisma__LibraryClient<Library>, Prisma__LibraryClient<LibraryGetPayload<T>>>

    /**
     * Delete zero or more Libraries.
     * @param {LibraryDeleteManyArgs} args - Arguments to filter Libraries to delete.
     * @example
     * // Delete a few Libraries
     * const { count } = await prisma.library.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends LibraryDeleteManyArgs>(
      args?: SelectSubset<T, LibraryDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Libraries.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Libraries
     * const library = await prisma.library.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends LibraryUpdateManyArgs>(
      args: SelectSubset<T, LibraryUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Library.
     * @param {LibraryUpsertArgs} args - Arguments to update or create a Library.
     * @example
     * // Update or create a Library
     * const library = await prisma.library.upsert({
     *   create: {
     *     // ... data to create a Library
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Library we want to update
     *   }
     * })
    **/
    upsert<T extends LibraryUpsertArgs>(
      args: SelectSubset<T, LibraryUpsertArgs>
    ): CheckSelect<T, Prisma__LibraryClient<Library>, Prisma__LibraryClient<LibraryGetPayload<T>>>

    /**
     * Count the number of Libraries.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryCountArgs} args - Arguments to filter Libraries to count.
     * @example
     * // Count the number of Libraries
     * const count = await prisma.library.count({
     *   where: {
     *     // ... the filter for the Libraries we want to count
     *   }
     * })
    **/
    count<T extends LibraryCountArgs>(
      args?: Subset<T, LibraryCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], LibraryCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Library.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends LibraryAggregateArgs>(args: Subset<T, LibraryAggregateArgs>): PrismaPromise<GetLibraryAggregateType<T>>

    /**
     * Group by Library.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends LibraryGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: LibraryGroupByArgs['orderBy'] }
        : { orderBy?: LibraryGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, LibraryGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetLibraryGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Library.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__LibraryClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    spaces<T extends SpaceFindManyArgs = {}>(args?: Subset<T, SpaceFindManyArgs>): CheckSelect<T, PrismaPromise<Array<Space>>, PrismaPromise<Array<SpaceGetPayload<T>>>>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Library findUnique
   */
  export type LibraryFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * Throw an Error if a Library can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Library to fetch.
     * 
    **/
    where: LibraryWhereUniqueInput
  }


  /**
   * Library findFirst
   */
  export type LibraryFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * Throw an Error if a Library can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Library to fetch.
     * 
    **/
    where?: LibraryWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Libraries to fetch.
     * 
    **/
    orderBy?: Enumerable<LibraryOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Libraries.
     * 
    **/
    cursor?: LibraryWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Libraries from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Libraries.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Libraries.
     * 
    **/
    distinct?: Enumerable<LibraryScalarFieldEnum>
  }


  /**
   * Library findMany
   */
  export type LibraryFindManyArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * Filter, which Libraries to fetch.
     * 
    **/
    where?: LibraryWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Libraries to fetch.
     * 
    **/
    orderBy?: Enumerable<LibraryOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Libraries.
     * 
    **/
    cursor?: LibraryWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Libraries from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Libraries.
     * 
    **/
    skip?: number
    distinct?: Enumerable<LibraryScalarFieldEnum>
  }


  /**
   * Library create
   */
  export type LibraryCreateArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * The data needed to create a Library.
     * 
    **/
    data: XOR<LibraryCreateInput, LibraryUncheckedCreateInput>
  }


  /**
   * Library update
   */
  export type LibraryUpdateArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * The data needed to update a Library.
     * 
    **/
    data: XOR<LibraryUpdateInput, LibraryUncheckedUpdateInput>
    /**
     * Choose, which Library to update.
     * 
    **/
    where: LibraryWhereUniqueInput
  }


  /**
   * Library updateMany
   */
  export type LibraryUpdateManyArgs = {
    /**
     * The data used to update Libraries.
     * 
    **/
    data: XOR<LibraryUpdateManyMutationInput, LibraryUncheckedUpdateManyInput>
    /**
     * Filter which Libraries to update
     * 
    **/
    where?: LibraryWhereInput
  }


  /**
   * Library upsert
   */
  export type LibraryUpsertArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * The filter to search for the Library to update in case it exists.
     * 
    **/
    where: LibraryWhereUniqueInput
    /**
     * In case the Library found by the `where` argument doesn't exist, create a new Library with this data.
     * 
    **/
    create: XOR<LibraryCreateInput, LibraryUncheckedCreateInput>
    /**
     * In case the Library was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<LibraryUpdateInput, LibraryUncheckedUpdateInput>
  }


  /**
   * Library delete
   */
  export type LibraryDeleteArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
    /**
     * Filter which Library to delete.
     * 
    **/
    where: LibraryWhereUniqueInput
  }


  /**
   * Library deleteMany
   */
  export type LibraryDeleteManyArgs = {
    /**
     * Filter which Libraries to delete
     * 
    **/
    where?: LibraryWhereInput
  }


  /**
   * Library without action
   */
  export type LibraryArgs = {
    /**
     * Select specific fields to fetch from the Library
     * 
    **/
    select?: LibrarySelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LibraryInclude | null
  }



  /**
   * Model LibraryStatistics
   */


  export type AggregateLibraryStatistics = {
    _count: LibraryStatisticsCountAggregateOutputType | null
    _avg: LibraryStatisticsAvgAggregateOutputType | null
    _sum: LibraryStatisticsSumAggregateOutputType | null
    _min: LibraryStatisticsMinAggregateOutputType | null
    _max: LibraryStatisticsMaxAggregateOutputType | null
  }

  export type LibraryStatisticsAvgAggregateOutputType = {
    id: number | null
    library_id: number | null
    total_file_count: number | null
  }

  export type LibraryStatisticsSumAggregateOutputType = {
    id: number | null
    library_id: number | null
    total_file_count: number | null
  }

  export type LibraryStatisticsMinAggregateOutputType = {
    id: number | null
    date_captured: Date | null
    library_id: number | null
    total_file_count: number | null
    total_bytes_used: string | null
    total_byte_capacity: string | null
    total_unique_bytes: string | null
  }

  export type LibraryStatisticsMaxAggregateOutputType = {
    id: number | null
    date_captured: Date | null
    library_id: number | null
    total_file_count: number | null
    total_bytes_used: string | null
    total_byte_capacity: string | null
    total_unique_bytes: string | null
  }

  export type LibraryStatisticsCountAggregateOutputType = {
    id: number
    date_captured: number
    library_id: number
    total_file_count: number
    total_bytes_used: number
    total_byte_capacity: number
    total_unique_bytes: number
    _all: number
  }


  export type LibraryStatisticsAvgAggregateInputType = {
    id?: true
    library_id?: true
    total_file_count?: true
  }

  export type LibraryStatisticsSumAggregateInputType = {
    id?: true
    library_id?: true
    total_file_count?: true
  }

  export type LibraryStatisticsMinAggregateInputType = {
    id?: true
    date_captured?: true
    library_id?: true
    total_file_count?: true
    total_bytes_used?: true
    total_byte_capacity?: true
    total_unique_bytes?: true
  }

  export type LibraryStatisticsMaxAggregateInputType = {
    id?: true
    date_captured?: true
    library_id?: true
    total_file_count?: true
    total_bytes_used?: true
    total_byte_capacity?: true
    total_unique_bytes?: true
  }

  export type LibraryStatisticsCountAggregateInputType = {
    id?: true
    date_captured?: true
    library_id?: true
    total_file_count?: true
    total_bytes_used?: true
    total_byte_capacity?: true
    total_unique_bytes?: true
    _all?: true
  }

  export type LibraryStatisticsAggregateArgs = {
    /**
     * Filter which LibraryStatistics to aggregate.
     * 
    **/
    where?: LibraryStatisticsWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of LibraryStatistics to fetch.
     * 
    **/
    orderBy?: Enumerable<LibraryStatisticsOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: LibraryStatisticsWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` LibraryStatistics from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` LibraryStatistics.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned LibraryStatistics
    **/
    _count?: true | LibraryStatisticsCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: LibraryStatisticsAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: LibraryStatisticsSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: LibraryStatisticsMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: LibraryStatisticsMaxAggregateInputType
  }

  export type GetLibraryStatisticsAggregateType<T extends LibraryStatisticsAggregateArgs> = {
        [P in keyof T & keyof AggregateLibraryStatistics]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateLibraryStatistics[P]>
      : GetScalarType<T[P], AggregateLibraryStatistics[P]>
  }




  export type LibraryStatisticsGroupByArgs = {
    where?: LibraryStatisticsWhereInput
    orderBy?: Enumerable<LibraryStatisticsOrderByWithAggregationInput>
    by: Array<LibraryStatisticsScalarFieldEnum>
    having?: LibraryStatisticsScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: LibraryStatisticsCountAggregateInputType | true
    _avg?: LibraryStatisticsAvgAggregateInputType
    _sum?: LibraryStatisticsSumAggregateInputType
    _min?: LibraryStatisticsMinAggregateInputType
    _max?: LibraryStatisticsMaxAggregateInputType
  }


  export type LibraryStatisticsGroupByOutputType = {
    id: number
    date_captured: Date
    library_id: number
    total_file_count: number
    total_bytes_used: string
    total_byte_capacity: string
    total_unique_bytes: string
    _count: LibraryStatisticsCountAggregateOutputType | null
    _avg: LibraryStatisticsAvgAggregateOutputType | null
    _sum: LibraryStatisticsSumAggregateOutputType | null
    _min: LibraryStatisticsMinAggregateOutputType | null
    _max: LibraryStatisticsMaxAggregateOutputType | null
  }

  type GetLibraryStatisticsGroupByPayload<T extends LibraryStatisticsGroupByArgs> = PrismaPromise<
    Array<
      PickArray<LibraryStatisticsGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof LibraryStatisticsGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], LibraryStatisticsGroupByOutputType[P]>
            : GetScalarType<T[P], LibraryStatisticsGroupByOutputType[P]>
        }
      >
    >


  export type LibraryStatisticsSelect = {
    id?: boolean
    date_captured?: boolean
    library_id?: boolean
    total_file_count?: boolean
    total_bytes_used?: boolean
    total_byte_capacity?: boolean
    total_unique_bytes?: boolean
  }

  export type LibraryStatisticsGetPayload<
    S extends boolean | null | undefined | LibraryStatisticsArgs,
    U = keyof S
      > = S extends true
        ? LibraryStatistics
    : S extends undefined
    ? never
    : S extends LibraryStatisticsArgs | LibraryStatisticsFindManyArgs
    ?'include' extends U
    ? LibraryStatistics 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
    P extends keyof LibraryStatistics ? LibraryStatistics[P] : never
  } 
    : LibraryStatistics
  : LibraryStatistics


  type LibraryStatisticsCountArgs = Merge<
    Omit<LibraryStatisticsFindManyArgs, 'select' | 'include'> & {
      select?: LibraryStatisticsCountAggregateInputType | true
    }
  >

  export interface LibraryStatisticsDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one LibraryStatistics that matches the filter.
     * @param {LibraryStatisticsFindUniqueArgs} args - Arguments to find a LibraryStatistics
     * @example
     * // Get one LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends LibraryStatisticsFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, LibraryStatisticsFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'LibraryStatistics'> extends True ? CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics>, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T>>> : CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics | null >, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T> | null >>

    /**
     * Find the first LibraryStatistics that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryStatisticsFindFirstArgs} args - Arguments to find a LibraryStatistics
     * @example
     * // Get one LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends LibraryStatisticsFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, LibraryStatisticsFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'LibraryStatistics'> extends True ? CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics>, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T>>> : CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics | null >, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T> | null >>

    /**
     * Find zero or more LibraryStatistics that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryStatisticsFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.findMany()
     * 
     * // Get first 10 LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const libraryStatisticsWithIdOnly = await prisma.libraryStatistics.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends LibraryStatisticsFindManyArgs>(
      args?: SelectSubset<T, LibraryStatisticsFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<LibraryStatistics>>, PrismaPromise<Array<LibraryStatisticsGetPayload<T>>>>

    /**
     * Create a LibraryStatistics.
     * @param {LibraryStatisticsCreateArgs} args - Arguments to create a LibraryStatistics.
     * @example
     * // Create one LibraryStatistics
     * const LibraryStatistics = await prisma.libraryStatistics.create({
     *   data: {
     *     // ... data to create a LibraryStatistics
     *   }
     * })
     * 
    **/
    create<T extends LibraryStatisticsCreateArgs>(
      args: SelectSubset<T, LibraryStatisticsCreateArgs>
    ): CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics>, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T>>>

    /**
     * Delete a LibraryStatistics.
     * @param {LibraryStatisticsDeleteArgs} args - Arguments to delete one LibraryStatistics.
     * @example
     * // Delete one LibraryStatistics
     * const LibraryStatistics = await prisma.libraryStatistics.delete({
     *   where: {
     *     // ... filter to delete one LibraryStatistics
     *   }
     * })
     * 
    **/
    delete<T extends LibraryStatisticsDeleteArgs>(
      args: SelectSubset<T, LibraryStatisticsDeleteArgs>
    ): CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics>, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T>>>

    /**
     * Update one LibraryStatistics.
     * @param {LibraryStatisticsUpdateArgs} args - Arguments to update one LibraryStatistics.
     * @example
     * // Update one LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends LibraryStatisticsUpdateArgs>(
      args: SelectSubset<T, LibraryStatisticsUpdateArgs>
    ): CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics>, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T>>>

    /**
     * Delete zero or more LibraryStatistics.
     * @param {LibraryStatisticsDeleteManyArgs} args - Arguments to filter LibraryStatistics to delete.
     * @example
     * // Delete a few LibraryStatistics
     * const { count } = await prisma.libraryStatistics.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends LibraryStatisticsDeleteManyArgs>(
      args?: SelectSubset<T, LibraryStatisticsDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more LibraryStatistics.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryStatisticsUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends LibraryStatisticsUpdateManyArgs>(
      args: SelectSubset<T, LibraryStatisticsUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one LibraryStatistics.
     * @param {LibraryStatisticsUpsertArgs} args - Arguments to update or create a LibraryStatistics.
     * @example
     * // Update or create a LibraryStatistics
     * const libraryStatistics = await prisma.libraryStatistics.upsert({
     *   create: {
     *     // ... data to create a LibraryStatistics
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the LibraryStatistics we want to update
     *   }
     * })
    **/
    upsert<T extends LibraryStatisticsUpsertArgs>(
      args: SelectSubset<T, LibraryStatisticsUpsertArgs>
    ): CheckSelect<T, Prisma__LibraryStatisticsClient<LibraryStatistics>, Prisma__LibraryStatisticsClient<LibraryStatisticsGetPayload<T>>>

    /**
     * Count the number of LibraryStatistics.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryStatisticsCountArgs} args - Arguments to filter LibraryStatistics to count.
     * @example
     * // Count the number of LibraryStatistics
     * const count = await prisma.libraryStatistics.count({
     *   where: {
     *     // ... the filter for the LibraryStatistics we want to count
     *   }
     * })
    **/
    count<T extends LibraryStatisticsCountArgs>(
      args?: Subset<T, LibraryStatisticsCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], LibraryStatisticsCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a LibraryStatistics.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryStatisticsAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends LibraryStatisticsAggregateArgs>(args: Subset<T, LibraryStatisticsAggregateArgs>): PrismaPromise<GetLibraryStatisticsAggregateType<T>>

    /**
     * Group by LibraryStatistics.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LibraryStatisticsGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends LibraryStatisticsGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: LibraryStatisticsGroupByArgs['orderBy'] }
        : { orderBy?: LibraryStatisticsGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, LibraryStatisticsGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetLibraryStatisticsGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for LibraryStatistics.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__LibraryStatisticsClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';


    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * LibraryStatistics findUnique
   */
  export type LibraryStatisticsFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * Throw an Error if a LibraryStatistics can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which LibraryStatistics to fetch.
     * 
    **/
    where: LibraryStatisticsWhereUniqueInput
  }


  /**
   * LibraryStatistics findFirst
   */
  export type LibraryStatisticsFindFirstArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * Throw an Error if a LibraryStatistics can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which LibraryStatistics to fetch.
     * 
    **/
    where?: LibraryStatisticsWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of LibraryStatistics to fetch.
     * 
    **/
    orderBy?: Enumerable<LibraryStatisticsOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for LibraryStatistics.
     * 
    **/
    cursor?: LibraryStatisticsWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` LibraryStatistics from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` LibraryStatistics.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of LibraryStatistics.
     * 
    **/
    distinct?: Enumerable<LibraryStatisticsScalarFieldEnum>
  }


  /**
   * LibraryStatistics findMany
   */
  export type LibraryStatisticsFindManyArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * Filter, which LibraryStatistics to fetch.
     * 
    **/
    where?: LibraryStatisticsWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of LibraryStatistics to fetch.
     * 
    **/
    orderBy?: Enumerable<LibraryStatisticsOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing LibraryStatistics.
     * 
    **/
    cursor?: LibraryStatisticsWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` LibraryStatistics from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` LibraryStatistics.
     * 
    **/
    skip?: number
    distinct?: Enumerable<LibraryStatisticsScalarFieldEnum>
  }


  /**
   * LibraryStatistics create
   */
  export type LibraryStatisticsCreateArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * The data needed to create a LibraryStatistics.
     * 
    **/
    data: XOR<LibraryStatisticsCreateInput, LibraryStatisticsUncheckedCreateInput>
  }


  /**
   * LibraryStatistics update
   */
  export type LibraryStatisticsUpdateArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * The data needed to update a LibraryStatistics.
     * 
    **/
    data: XOR<LibraryStatisticsUpdateInput, LibraryStatisticsUncheckedUpdateInput>
    /**
     * Choose, which LibraryStatistics to update.
     * 
    **/
    where: LibraryStatisticsWhereUniqueInput
  }


  /**
   * LibraryStatistics updateMany
   */
  export type LibraryStatisticsUpdateManyArgs = {
    /**
     * The data used to update LibraryStatistics.
     * 
    **/
    data: XOR<LibraryStatisticsUpdateManyMutationInput, LibraryStatisticsUncheckedUpdateManyInput>
    /**
     * Filter which LibraryStatistics to update
     * 
    **/
    where?: LibraryStatisticsWhereInput
  }


  /**
   * LibraryStatistics upsert
   */
  export type LibraryStatisticsUpsertArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * The filter to search for the LibraryStatistics to update in case it exists.
     * 
    **/
    where: LibraryStatisticsWhereUniqueInput
    /**
     * In case the LibraryStatistics found by the `where` argument doesn't exist, create a new LibraryStatistics with this data.
     * 
    **/
    create: XOR<LibraryStatisticsCreateInput, LibraryStatisticsUncheckedCreateInput>
    /**
     * In case the LibraryStatistics was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<LibraryStatisticsUpdateInput, LibraryStatisticsUncheckedUpdateInput>
  }


  /**
   * LibraryStatistics delete
   */
  export type LibraryStatisticsDeleteArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
    /**
     * Filter which LibraryStatistics to delete.
     * 
    **/
    where: LibraryStatisticsWhereUniqueInput
  }


  /**
   * LibraryStatistics deleteMany
   */
  export type LibraryStatisticsDeleteManyArgs = {
    /**
     * Filter which LibraryStatistics to delete
     * 
    **/
    where?: LibraryStatisticsWhereInput
  }


  /**
   * LibraryStatistics without action
   */
  export type LibraryStatisticsArgs = {
    /**
     * Select specific fields to fetch from the LibraryStatistics
     * 
    **/
    select?: LibraryStatisticsSelect | null
  }



  /**
   * Model Client
   */


  export type AggregateClient = {
    _count: ClientCountAggregateOutputType | null
    _avg: ClientAvgAggregateOutputType | null
    _sum: ClientSumAggregateOutputType | null
    _min: ClientMinAggregateOutputType | null
    _max: ClientMaxAggregateOutputType | null
  }

  export type ClientAvgAggregateOutputType = {
    id: number | null
    platform: number | null
  }

  export type ClientSumAggregateOutputType = {
    id: number | null
    platform: number | null
  }

  export type ClientMinAggregateOutputType = {
    id: number | null
    uuid: string | null
    name: string | null
    platform: number | null
    version: string | null
    online: boolean | null
    last_seen: Date | null
    timezone: string | null
    date_created: Date | null
  }

  export type ClientMaxAggregateOutputType = {
    id: number | null
    uuid: string | null
    name: string | null
    platform: number | null
    version: string | null
    online: boolean | null
    last_seen: Date | null
    timezone: string | null
    date_created: Date | null
  }

  export type ClientCountAggregateOutputType = {
    id: number
    uuid: number
    name: number
    platform: number
    version: number
    online: number
    last_seen: number
    timezone: number
    date_created: number
    _all: number
  }


  export type ClientAvgAggregateInputType = {
    id?: true
    platform?: true
  }

  export type ClientSumAggregateInputType = {
    id?: true
    platform?: true
  }

  export type ClientMinAggregateInputType = {
    id?: true
    uuid?: true
    name?: true
    platform?: true
    version?: true
    online?: true
    last_seen?: true
    timezone?: true
    date_created?: true
  }

  export type ClientMaxAggregateInputType = {
    id?: true
    uuid?: true
    name?: true
    platform?: true
    version?: true
    online?: true
    last_seen?: true
    timezone?: true
    date_created?: true
  }

  export type ClientCountAggregateInputType = {
    id?: true
    uuid?: true
    name?: true
    platform?: true
    version?: true
    online?: true
    last_seen?: true
    timezone?: true
    date_created?: true
    _all?: true
  }

  export type ClientAggregateArgs = {
    /**
     * Filter which Client to aggregate.
     * 
    **/
    where?: ClientWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Clients to fetch.
     * 
    **/
    orderBy?: Enumerable<ClientOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: ClientWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Clients from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Clients.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Clients
    **/
    _count?: true | ClientCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: ClientAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: ClientSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: ClientMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: ClientMaxAggregateInputType
  }

  export type GetClientAggregateType<T extends ClientAggregateArgs> = {
        [P in keyof T & keyof AggregateClient]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateClient[P]>
      : GetScalarType<T[P], AggregateClient[P]>
  }




  export type ClientGroupByArgs = {
    where?: ClientWhereInput
    orderBy?: Enumerable<ClientOrderByWithAggregationInput>
    by: Array<ClientScalarFieldEnum>
    having?: ClientScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: ClientCountAggregateInputType | true
    _avg?: ClientAvgAggregateInputType
    _sum?: ClientSumAggregateInputType
    _min?: ClientMinAggregateInputType
    _max?: ClientMaxAggregateInputType
  }


  export type ClientGroupByOutputType = {
    id: number
    uuid: string
    name: string
    platform: number
    version: string | null
    online: boolean | null
    last_seen: Date
    timezone: string | null
    date_created: Date
    _count: ClientCountAggregateOutputType | null
    _avg: ClientAvgAggregateOutputType | null
    _sum: ClientSumAggregateOutputType | null
    _min: ClientMinAggregateOutputType | null
    _max: ClientMaxAggregateOutputType | null
  }

  type GetClientGroupByPayload<T extends ClientGroupByArgs> = PrismaPromise<
    Array<
      PickArray<ClientGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof ClientGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], ClientGroupByOutputType[P]>
            : GetScalarType<T[P], ClientGroupByOutputType[P]>
        }
      >
    >


  export type ClientSelect = {
    id?: boolean
    uuid?: boolean
    name?: boolean
    platform?: boolean
    version?: boolean
    online?: boolean
    last_seen?: boolean
    timezone?: boolean
    date_created?: boolean
    jobs?: boolean | JobFindManyArgs
    _count?: boolean | ClientCountOutputTypeArgs
  }

  export type ClientInclude = {
    jobs?: boolean | JobFindManyArgs
    _count?: boolean | ClientCountOutputTypeArgs
  }

  export type ClientGetPayload<
    S extends boolean | null | undefined | ClientArgs,
    U = keyof S
      > = S extends true
        ? Client
    : S extends undefined
    ? never
    : S extends ClientArgs | ClientFindManyArgs
    ?'include' extends U
    ? Client  & {
    [P in TrueKeys<S['include']>]:
        P extends 'jobs' ? Array < JobGetPayload<S['include'][P]>>  :
        P extends '_count' ? ClientCountOutputTypeGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'jobs' ? Array < JobGetPayload<S['select'][P]>>  :
        P extends '_count' ? ClientCountOutputTypeGetPayload<S['select'][P]> :  P extends keyof Client ? Client[P] : never
  } 
    : Client
  : Client


  type ClientCountArgs = Merge<
    Omit<ClientFindManyArgs, 'select' | 'include'> & {
      select?: ClientCountAggregateInputType | true
    }
  >

  export interface ClientDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Client that matches the filter.
     * @param {ClientFindUniqueArgs} args - Arguments to find a Client
     * @example
     * // Get one Client
     * const client = await prisma.client.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends ClientFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, ClientFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Client'> extends True ? CheckSelect<T, Prisma__ClientClient<Client>, Prisma__ClientClient<ClientGetPayload<T>>> : CheckSelect<T, Prisma__ClientClient<Client | null >, Prisma__ClientClient<ClientGetPayload<T> | null >>

    /**
     * Find the first Client that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {ClientFindFirstArgs} args - Arguments to find a Client
     * @example
     * // Get one Client
     * const client = await prisma.client.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends ClientFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, ClientFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Client'> extends True ? CheckSelect<T, Prisma__ClientClient<Client>, Prisma__ClientClient<ClientGetPayload<T>>> : CheckSelect<T, Prisma__ClientClient<Client | null >, Prisma__ClientClient<ClientGetPayload<T> | null >>

    /**
     * Find zero or more Clients that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {ClientFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Clients
     * const clients = await prisma.client.findMany()
     * 
     * // Get first 10 Clients
     * const clients = await prisma.client.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const clientWithIdOnly = await prisma.client.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends ClientFindManyArgs>(
      args?: SelectSubset<T, ClientFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Client>>, PrismaPromise<Array<ClientGetPayload<T>>>>

    /**
     * Create a Client.
     * @param {ClientCreateArgs} args - Arguments to create a Client.
     * @example
     * // Create one Client
     * const Client = await prisma.client.create({
     *   data: {
     *     // ... data to create a Client
     *   }
     * })
     * 
    **/
    create<T extends ClientCreateArgs>(
      args: SelectSubset<T, ClientCreateArgs>
    ): CheckSelect<T, Prisma__ClientClient<Client>, Prisma__ClientClient<ClientGetPayload<T>>>

    /**
     * Delete a Client.
     * @param {ClientDeleteArgs} args - Arguments to delete one Client.
     * @example
     * // Delete one Client
     * const Client = await prisma.client.delete({
     *   where: {
     *     // ... filter to delete one Client
     *   }
     * })
     * 
    **/
    delete<T extends ClientDeleteArgs>(
      args: SelectSubset<T, ClientDeleteArgs>
    ): CheckSelect<T, Prisma__ClientClient<Client>, Prisma__ClientClient<ClientGetPayload<T>>>

    /**
     * Update one Client.
     * @param {ClientUpdateArgs} args - Arguments to update one Client.
     * @example
     * // Update one Client
     * const client = await prisma.client.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends ClientUpdateArgs>(
      args: SelectSubset<T, ClientUpdateArgs>
    ): CheckSelect<T, Prisma__ClientClient<Client>, Prisma__ClientClient<ClientGetPayload<T>>>

    /**
     * Delete zero or more Clients.
     * @param {ClientDeleteManyArgs} args - Arguments to filter Clients to delete.
     * @example
     * // Delete a few Clients
     * const { count } = await prisma.client.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends ClientDeleteManyArgs>(
      args?: SelectSubset<T, ClientDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Clients.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {ClientUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Clients
     * const client = await prisma.client.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends ClientUpdateManyArgs>(
      args: SelectSubset<T, ClientUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Client.
     * @param {ClientUpsertArgs} args - Arguments to update or create a Client.
     * @example
     * // Update or create a Client
     * const client = await prisma.client.upsert({
     *   create: {
     *     // ... data to create a Client
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Client we want to update
     *   }
     * })
    **/
    upsert<T extends ClientUpsertArgs>(
      args: SelectSubset<T, ClientUpsertArgs>
    ): CheckSelect<T, Prisma__ClientClient<Client>, Prisma__ClientClient<ClientGetPayload<T>>>

    /**
     * Count the number of Clients.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {ClientCountArgs} args - Arguments to filter Clients to count.
     * @example
     * // Count the number of Clients
     * const count = await prisma.client.count({
     *   where: {
     *     // ... the filter for the Clients we want to count
     *   }
     * })
    **/
    count<T extends ClientCountArgs>(
      args?: Subset<T, ClientCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], ClientCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Client.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {ClientAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends ClientAggregateArgs>(args: Subset<T, ClientAggregateArgs>): PrismaPromise<GetClientAggregateType<T>>

    /**
     * Group by Client.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {ClientGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends ClientGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: ClientGroupByArgs['orderBy'] }
        : { orderBy?: ClientGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, ClientGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetClientGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Client.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__ClientClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    jobs<T extends JobFindManyArgs = {}>(args?: Subset<T, JobFindManyArgs>): CheckSelect<T, PrismaPromise<Array<Job>>, PrismaPromise<Array<JobGetPayload<T>>>>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Client findUnique
   */
  export type ClientFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * Throw an Error if a Client can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Client to fetch.
     * 
    **/
    where: ClientWhereUniqueInput
  }


  /**
   * Client findFirst
   */
  export type ClientFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * Throw an Error if a Client can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Client to fetch.
     * 
    **/
    where?: ClientWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Clients to fetch.
     * 
    **/
    orderBy?: Enumerable<ClientOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Clients.
     * 
    **/
    cursor?: ClientWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Clients from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Clients.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Clients.
     * 
    **/
    distinct?: Enumerable<ClientScalarFieldEnum>
  }


  /**
   * Client findMany
   */
  export type ClientFindManyArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * Filter, which Clients to fetch.
     * 
    **/
    where?: ClientWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Clients to fetch.
     * 
    **/
    orderBy?: Enumerable<ClientOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Clients.
     * 
    **/
    cursor?: ClientWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Clients from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Clients.
     * 
    **/
    skip?: number
    distinct?: Enumerable<ClientScalarFieldEnum>
  }


  /**
   * Client create
   */
  export type ClientCreateArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * The data needed to create a Client.
     * 
    **/
    data: XOR<ClientCreateInput, ClientUncheckedCreateInput>
  }


  /**
   * Client update
   */
  export type ClientUpdateArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * The data needed to update a Client.
     * 
    **/
    data: XOR<ClientUpdateInput, ClientUncheckedUpdateInput>
    /**
     * Choose, which Client to update.
     * 
    **/
    where: ClientWhereUniqueInput
  }


  /**
   * Client updateMany
   */
  export type ClientUpdateManyArgs = {
    /**
     * The data used to update Clients.
     * 
    **/
    data: XOR<ClientUpdateManyMutationInput, ClientUncheckedUpdateManyInput>
    /**
     * Filter which Clients to update
     * 
    **/
    where?: ClientWhereInput
  }


  /**
   * Client upsert
   */
  export type ClientUpsertArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * The filter to search for the Client to update in case it exists.
     * 
    **/
    where: ClientWhereUniqueInput
    /**
     * In case the Client found by the `where` argument doesn't exist, create a new Client with this data.
     * 
    **/
    create: XOR<ClientCreateInput, ClientUncheckedCreateInput>
    /**
     * In case the Client was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<ClientUpdateInput, ClientUncheckedUpdateInput>
  }


  /**
   * Client delete
   */
  export type ClientDeleteArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
    /**
     * Filter which Client to delete.
     * 
    **/
    where: ClientWhereUniqueInput
  }


  /**
   * Client deleteMany
   */
  export type ClientDeleteManyArgs = {
    /**
     * Filter which Clients to delete
     * 
    **/
    where?: ClientWhereInput
  }


  /**
   * Client without action
   */
  export type ClientArgs = {
    /**
     * Select specific fields to fetch from the Client
     * 
    **/
    select?: ClientSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: ClientInclude | null
  }



  /**
   * Model Location
   */


  export type AggregateLocation = {
    _count: LocationCountAggregateOutputType | null
    _avg: LocationAvgAggregateOutputType | null
    _sum: LocationSumAggregateOutputType | null
    _min: LocationMinAggregateOutputType | null
    _max: LocationMaxAggregateOutputType | null
  }

  export type LocationAvgAggregateOutputType = {
    id: number | null
    total_capacity: number | null
    available_capacity: number | null
  }

  export type LocationSumAggregateOutputType = {
    id: number | null
    total_capacity: number | null
    available_capacity: number | null
  }

  export type LocationMinAggregateOutputType = {
    id: number | null
    name: string | null
    path: string | null
    total_capacity: number | null
    available_capacity: number | null
    is_removable: boolean | null
    is_ejectable: boolean | null
    is_root_filesystem: boolean | null
    is_online: boolean | null
    date_created: Date | null
  }

  export type LocationMaxAggregateOutputType = {
    id: number | null
    name: string | null
    path: string | null
    total_capacity: number | null
    available_capacity: number | null
    is_removable: boolean | null
    is_ejectable: boolean | null
    is_root_filesystem: boolean | null
    is_online: boolean | null
    date_created: Date | null
  }

  export type LocationCountAggregateOutputType = {
    id: number
    name: number
    path: number
    total_capacity: number
    available_capacity: number
    is_removable: number
    is_ejectable: number
    is_root_filesystem: number
    is_online: number
    date_created: number
    _all: number
  }


  export type LocationAvgAggregateInputType = {
    id?: true
    total_capacity?: true
    available_capacity?: true
  }

  export type LocationSumAggregateInputType = {
    id?: true
    total_capacity?: true
    available_capacity?: true
  }

  export type LocationMinAggregateInputType = {
    id?: true
    name?: true
    path?: true
    total_capacity?: true
    available_capacity?: true
    is_removable?: true
    is_ejectable?: true
    is_root_filesystem?: true
    is_online?: true
    date_created?: true
  }

  export type LocationMaxAggregateInputType = {
    id?: true
    name?: true
    path?: true
    total_capacity?: true
    available_capacity?: true
    is_removable?: true
    is_ejectable?: true
    is_root_filesystem?: true
    is_online?: true
    date_created?: true
  }

  export type LocationCountAggregateInputType = {
    id?: true
    name?: true
    path?: true
    total_capacity?: true
    available_capacity?: true
    is_removable?: true
    is_ejectable?: true
    is_root_filesystem?: true
    is_online?: true
    date_created?: true
    _all?: true
  }

  export type LocationAggregateArgs = {
    /**
     * Filter which Location to aggregate.
     * 
    **/
    where?: LocationWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Locations to fetch.
     * 
    **/
    orderBy?: Enumerable<LocationOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: LocationWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Locations from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Locations.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Locations
    **/
    _count?: true | LocationCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: LocationAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: LocationSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: LocationMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: LocationMaxAggregateInputType
  }

  export type GetLocationAggregateType<T extends LocationAggregateArgs> = {
        [P in keyof T & keyof AggregateLocation]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateLocation[P]>
      : GetScalarType<T[P], AggregateLocation[P]>
  }




  export type LocationGroupByArgs = {
    where?: LocationWhereInput
    orderBy?: Enumerable<LocationOrderByWithAggregationInput>
    by: Array<LocationScalarFieldEnum>
    having?: LocationScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: LocationCountAggregateInputType | true
    _avg?: LocationAvgAggregateInputType
    _sum?: LocationSumAggregateInputType
    _min?: LocationMinAggregateInputType
    _max?: LocationMaxAggregateInputType
  }


  export type LocationGroupByOutputType = {
    id: number
    name: string | null
    path: string | null
    total_capacity: number | null
    available_capacity: number | null
    is_removable: boolean
    is_ejectable: boolean
    is_root_filesystem: boolean
    is_online: boolean
    date_created: Date
    _count: LocationCountAggregateOutputType | null
    _avg: LocationAvgAggregateOutputType | null
    _sum: LocationSumAggregateOutputType | null
    _min: LocationMinAggregateOutputType | null
    _max: LocationMaxAggregateOutputType | null
  }

  type GetLocationGroupByPayload<T extends LocationGroupByArgs> = PrismaPromise<
    Array<
      PickArray<LocationGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof LocationGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], LocationGroupByOutputType[P]>
            : GetScalarType<T[P], LocationGroupByOutputType[P]>
        }
      >
    >


  export type LocationSelect = {
    id?: boolean
    name?: boolean
    path?: boolean
    total_capacity?: boolean
    available_capacity?: boolean
    is_removable?: boolean
    is_ejectable?: boolean
    is_root_filesystem?: boolean
    is_online?: boolean
    date_created?: boolean
    files?: boolean | FileFindManyArgs
    _count?: boolean | LocationCountOutputTypeArgs
  }

  export type LocationInclude = {
    files?: boolean | FileFindManyArgs
    _count?: boolean | LocationCountOutputTypeArgs
  }

  export type LocationGetPayload<
    S extends boolean | null | undefined | LocationArgs,
    U = keyof S
      > = S extends true
        ? Location
    : S extends undefined
    ? never
    : S extends LocationArgs | LocationFindManyArgs
    ?'include' extends U
    ? Location  & {
    [P in TrueKeys<S['include']>]:
        P extends 'files' ? Array < FileGetPayload<S['include'][P]>>  :
        P extends '_count' ? LocationCountOutputTypeGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'files' ? Array < FileGetPayload<S['select'][P]>>  :
        P extends '_count' ? LocationCountOutputTypeGetPayload<S['select'][P]> :  P extends keyof Location ? Location[P] : never
  } 
    : Location
  : Location


  type LocationCountArgs = Merge<
    Omit<LocationFindManyArgs, 'select' | 'include'> & {
      select?: LocationCountAggregateInputType | true
    }
  >

  export interface LocationDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Location that matches the filter.
     * @param {LocationFindUniqueArgs} args - Arguments to find a Location
     * @example
     * // Get one Location
     * const location = await prisma.location.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends LocationFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, LocationFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Location'> extends True ? CheckSelect<T, Prisma__LocationClient<Location>, Prisma__LocationClient<LocationGetPayload<T>>> : CheckSelect<T, Prisma__LocationClient<Location | null >, Prisma__LocationClient<LocationGetPayload<T> | null >>

    /**
     * Find the first Location that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LocationFindFirstArgs} args - Arguments to find a Location
     * @example
     * // Get one Location
     * const location = await prisma.location.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends LocationFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, LocationFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Location'> extends True ? CheckSelect<T, Prisma__LocationClient<Location>, Prisma__LocationClient<LocationGetPayload<T>>> : CheckSelect<T, Prisma__LocationClient<Location | null >, Prisma__LocationClient<LocationGetPayload<T> | null >>

    /**
     * Find zero or more Locations that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LocationFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Locations
     * const locations = await prisma.location.findMany()
     * 
     * // Get first 10 Locations
     * const locations = await prisma.location.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const locationWithIdOnly = await prisma.location.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends LocationFindManyArgs>(
      args?: SelectSubset<T, LocationFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Location>>, PrismaPromise<Array<LocationGetPayload<T>>>>

    /**
     * Create a Location.
     * @param {LocationCreateArgs} args - Arguments to create a Location.
     * @example
     * // Create one Location
     * const Location = await prisma.location.create({
     *   data: {
     *     // ... data to create a Location
     *   }
     * })
     * 
    **/
    create<T extends LocationCreateArgs>(
      args: SelectSubset<T, LocationCreateArgs>
    ): CheckSelect<T, Prisma__LocationClient<Location>, Prisma__LocationClient<LocationGetPayload<T>>>

    /**
     * Delete a Location.
     * @param {LocationDeleteArgs} args - Arguments to delete one Location.
     * @example
     * // Delete one Location
     * const Location = await prisma.location.delete({
     *   where: {
     *     // ... filter to delete one Location
     *   }
     * })
     * 
    **/
    delete<T extends LocationDeleteArgs>(
      args: SelectSubset<T, LocationDeleteArgs>
    ): CheckSelect<T, Prisma__LocationClient<Location>, Prisma__LocationClient<LocationGetPayload<T>>>

    /**
     * Update one Location.
     * @param {LocationUpdateArgs} args - Arguments to update one Location.
     * @example
     * // Update one Location
     * const location = await prisma.location.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends LocationUpdateArgs>(
      args: SelectSubset<T, LocationUpdateArgs>
    ): CheckSelect<T, Prisma__LocationClient<Location>, Prisma__LocationClient<LocationGetPayload<T>>>

    /**
     * Delete zero or more Locations.
     * @param {LocationDeleteManyArgs} args - Arguments to filter Locations to delete.
     * @example
     * // Delete a few Locations
     * const { count } = await prisma.location.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends LocationDeleteManyArgs>(
      args?: SelectSubset<T, LocationDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Locations.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LocationUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Locations
     * const location = await prisma.location.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends LocationUpdateManyArgs>(
      args: SelectSubset<T, LocationUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Location.
     * @param {LocationUpsertArgs} args - Arguments to update or create a Location.
     * @example
     * // Update or create a Location
     * const location = await prisma.location.upsert({
     *   create: {
     *     // ... data to create a Location
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Location we want to update
     *   }
     * })
    **/
    upsert<T extends LocationUpsertArgs>(
      args: SelectSubset<T, LocationUpsertArgs>
    ): CheckSelect<T, Prisma__LocationClient<Location>, Prisma__LocationClient<LocationGetPayload<T>>>

    /**
     * Count the number of Locations.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LocationCountArgs} args - Arguments to filter Locations to count.
     * @example
     * // Count the number of Locations
     * const count = await prisma.location.count({
     *   where: {
     *     // ... the filter for the Locations we want to count
     *   }
     * })
    **/
    count<T extends LocationCountArgs>(
      args?: Subset<T, LocationCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], LocationCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Location.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LocationAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends LocationAggregateArgs>(args: Subset<T, LocationAggregateArgs>): PrismaPromise<GetLocationAggregateType<T>>

    /**
     * Group by Location.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {LocationGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends LocationGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: LocationGroupByArgs['orderBy'] }
        : { orderBy?: LocationGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, LocationGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetLocationGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Location.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__LocationClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    files<T extends FileFindManyArgs = {}>(args?: Subset<T, FileFindManyArgs>): CheckSelect<T, PrismaPromise<Array<File>>, PrismaPromise<Array<FileGetPayload<T>>>>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Location findUnique
   */
  export type LocationFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * Throw an Error if a Location can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Location to fetch.
     * 
    **/
    where: LocationWhereUniqueInput
  }


  /**
   * Location findFirst
   */
  export type LocationFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * Throw an Error if a Location can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Location to fetch.
     * 
    **/
    where?: LocationWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Locations to fetch.
     * 
    **/
    orderBy?: Enumerable<LocationOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Locations.
     * 
    **/
    cursor?: LocationWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Locations from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Locations.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Locations.
     * 
    **/
    distinct?: Enumerable<LocationScalarFieldEnum>
  }


  /**
   * Location findMany
   */
  export type LocationFindManyArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * Filter, which Locations to fetch.
     * 
    **/
    where?: LocationWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Locations to fetch.
     * 
    **/
    orderBy?: Enumerable<LocationOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Locations.
     * 
    **/
    cursor?: LocationWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Locations from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Locations.
     * 
    **/
    skip?: number
    distinct?: Enumerable<LocationScalarFieldEnum>
  }


  /**
   * Location create
   */
  export type LocationCreateArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * The data needed to create a Location.
     * 
    **/
    data: XOR<LocationCreateInput, LocationUncheckedCreateInput>
  }


  /**
   * Location update
   */
  export type LocationUpdateArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * The data needed to update a Location.
     * 
    **/
    data: XOR<LocationUpdateInput, LocationUncheckedUpdateInput>
    /**
     * Choose, which Location to update.
     * 
    **/
    where: LocationWhereUniqueInput
  }


  /**
   * Location updateMany
   */
  export type LocationUpdateManyArgs = {
    /**
     * The data used to update Locations.
     * 
    **/
    data: XOR<LocationUpdateManyMutationInput, LocationUncheckedUpdateManyInput>
    /**
     * Filter which Locations to update
     * 
    **/
    where?: LocationWhereInput
  }


  /**
   * Location upsert
   */
  export type LocationUpsertArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * The filter to search for the Location to update in case it exists.
     * 
    **/
    where: LocationWhereUniqueInput
    /**
     * In case the Location found by the `where` argument doesn't exist, create a new Location with this data.
     * 
    **/
    create: XOR<LocationCreateInput, LocationUncheckedCreateInput>
    /**
     * In case the Location was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<LocationUpdateInput, LocationUncheckedUpdateInput>
  }


  /**
   * Location delete
   */
  export type LocationDeleteArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
    /**
     * Filter which Location to delete.
     * 
    **/
    where: LocationWhereUniqueInput
  }


  /**
   * Location deleteMany
   */
  export type LocationDeleteManyArgs = {
    /**
     * Filter which Locations to delete
     * 
    **/
    where?: LocationWhereInput
  }


  /**
   * Location without action
   */
  export type LocationArgs = {
    /**
     * Select specific fields to fetch from the Location
     * 
    **/
    select?: LocationSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: LocationInclude | null
  }



  /**
   * Model File
   */


  export type AggregateFile = {
    _count: FileCountAggregateOutputType | null
    _avg: FileAvgAggregateOutputType | null
    _sum: FileSumAggregateOutputType | null
    _min: FileMinAggregateOutputType | null
    _max: FileMaxAggregateOutputType | null
  }

  export type FileAvgAggregateOutputType = {
    id: number | null
    location_id: number | null
    encryption: number | null
    parent_id: number | null
  }

  export type FileSumAggregateOutputType = {
    id: number | null
    location_id: number | null
    encryption: number | null
    parent_id: number | null
  }

  export type FileMinAggregateOutputType = {
    id: number | null
    is_dir: boolean | null
    location_id: number | null
    stem: string | null
    name: string | null
    extension: string | null
    quick_checksum: string | null
    full_checksum: string | null
    size_in_bytes: string | null
    encryption: number | null
    date_created: Date | null
    date_modified: Date | null
    date_indexed: Date | null
    ipfs_id: string | null
    parent_id: number | null
  }

  export type FileMaxAggregateOutputType = {
    id: number | null
    is_dir: boolean | null
    location_id: number | null
    stem: string | null
    name: string | null
    extension: string | null
    quick_checksum: string | null
    full_checksum: string | null
    size_in_bytes: string | null
    encryption: number | null
    date_created: Date | null
    date_modified: Date | null
    date_indexed: Date | null
    ipfs_id: string | null
    parent_id: number | null
  }

  export type FileCountAggregateOutputType = {
    id: number
    is_dir: number
    location_id: number
    stem: number
    name: number
    extension: number
    quick_checksum: number
    full_checksum: number
    size_in_bytes: number
    encryption: number
    date_created: number
    date_modified: number
    date_indexed: number
    ipfs_id: number
    parent_id: number
    _all: number
  }


  export type FileAvgAggregateInputType = {
    id?: true
    location_id?: true
    encryption?: true
    parent_id?: true
  }

  export type FileSumAggregateInputType = {
    id?: true
    location_id?: true
    encryption?: true
    parent_id?: true
  }

  export type FileMinAggregateInputType = {
    id?: true
    is_dir?: true
    location_id?: true
    stem?: true
    name?: true
    extension?: true
    quick_checksum?: true
    full_checksum?: true
    size_in_bytes?: true
    encryption?: true
    date_created?: true
    date_modified?: true
    date_indexed?: true
    ipfs_id?: true
    parent_id?: true
  }

  export type FileMaxAggregateInputType = {
    id?: true
    is_dir?: true
    location_id?: true
    stem?: true
    name?: true
    extension?: true
    quick_checksum?: true
    full_checksum?: true
    size_in_bytes?: true
    encryption?: true
    date_created?: true
    date_modified?: true
    date_indexed?: true
    ipfs_id?: true
    parent_id?: true
  }

  export type FileCountAggregateInputType = {
    id?: true
    is_dir?: true
    location_id?: true
    stem?: true
    name?: true
    extension?: true
    quick_checksum?: true
    full_checksum?: true
    size_in_bytes?: true
    encryption?: true
    date_created?: true
    date_modified?: true
    date_indexed?: true
    ipfs_id?: true
    parent_id?: true
    _all?: true
  }

  export type FileAggregateArgs = {
    /**
     * Filter which File to aggregate.
     * 
    **/
    where?: FileWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Files to fetch.
     * 
    **/
    orderBy?: Enumerable<FileOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: FileWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Files from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Files.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Files
    **/
    _count?: true | FileCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: FileAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: FileSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: FileMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: FileMaxAggregateInputType
  }

  export type GetFileAggregateType<T extends FileAggregateArgs> = {
        [P in keyof T & keyof AggregateFile]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateFile[P]>
      : GetScalarType<T[P], AggregateFile[P]>
  }




  export type FileGroupByArgs = {
    where?: FileWhereInput
    orderBy?: Enumerable<FileOrderByWithAggregationInput>
    by: Array<FileScalarFieldEnum>
    having?: FileScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: FileCountAggregateInputType | true
    _avg?: FileAvgAggregateInputType
    _sum?: FileSumAggregateInputType
    _min?: FileMinAggregateInputType
    _max?: FileMaxAggregateInputType
  }


  export type FileGroupByOutputType = {
    id: number
    is_dir: boolean
    location_id: number
    stem: string
    name: string
    extension: string | null
    quick_checksum: string | null
    full_checksum: string | null
    size_in_bytes: string
    encryption: number
    date_created: Date
    date_modified: Date
    date_indexed: Date
    ipfs_id: string | null
    parent_id: number | null
    _count: FileCountAggregateOutputType | null
    _avg: FileAvgAggregateOutputType | null
    _sum: FileSumAggregateOutputType | null
    _min: FileMinAggregateOutputType | null
    _max: FileMaxAggregateOutputType | null
  }

  type GetFileGroupByPayload<T extends FileGroupByArgs> = PrismaPromise<
    Array<
      PickArray<FileGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof FileGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], FileGroupByOutputType[P]>
            : GetScalarType<T[P], FileGroupByOutputType[P]>
        }
      >
    >


  export type FileSelect = {
    id?: boolean
    is_dir?: boolean
    location_id?: boolean
    stem?: boolean
    name?: boolean
    extension?: boolean
    quick_checksum?: boolean
    full_checksum?: boolean
    size_in_bytes?: boolean
    encryption?: boolean
    date_created?: boolean
    date_modified?: boolean
    date_indexed?: boolean
    ipfs_id?: boolean
    location?: boolean | LocationArgs
    parent?: boolean | FileArgs
    parent_id?: boolean
    children?: boolean | FileFindManyArgs
    file_tags?: boolean | TagOnFileFindManyArgs
    _count?: boolean | FileCountOutputTypeArgs
  }

  export type FileInclude = {
    location?: boolean | LocationArgs
    parent?: boolean | FileArgs
    children?: boolean | FileFindManyArgs
    file_tags?: boolean | TagOnFileFindManyArgs
    _count?: boolean | FileCountOutputTypeArgs
  }

  export type FileGetPayload<
    S extends boolean | null | undefined | FileArgs,
    U = keyof S
      > = S extends true
        ? File
    : S extends undefined
    ? never
    : S extends FileArgs | FileFindManyArgs
    ?'include' extends U
    ? File  & {
    [P in TrueKeys<S['include']>]:
        P extends 'location' ? LocationGetPayload<S['include'][P]> | null :
        P extends 'parent' ? FileGetPayload<S['include'][P]> | null :
        P extends 'children' ? Array < FileGetPayload<S['include'][P]>>  :
        P extends 'file_tags' ? Array < TagOnFileGetPayload<S['include'][P]>>  :
        P extends '_count' ? FileCountOutputTypeGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'location' ? LocationGetPayload<S['select'][P]> | null :
        P extends 'parent' ? FileGetPayload<S['select'][P]> | null :
        P extends 'children' ? Array < FileGetPayload<S['select'][P]>>  :
        P extends 'file_tags' ? Array < TagOnFileGetPayload<S['select'][P]>>  :
        P extends '_count' ? FileCountOutputTypeGetPayload<S['select'][P]> :  P extends keyof File ? File[P] : never
  } 
    : File
  : File


  type FileCountArgs = Merge<
    Omit<FileFindManyArgs, 'select' | 'include'> & {
      select?: FileCountAggregateInputType | true
    }
  >

  export interface FileDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one File that matches the filter.
     * @param {FileFindUniqueArgs} args - Arguments to find a File
     * @example
     * // Get one File
     * const file = await prisma.file.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends FileFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, FileFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'File'> extends True ? CheckSelect<T, Prisma__FileClient<File>, Prisma__FileClient<FileGetPayload<T>>> : CheckSelect<T, Prisma__FileClient<File | null >, Prisma__FileClient<FileGetPayload<T> | null >>

    /**
     * Find the first File that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {FileFindFirstArgs} args - Arguments to find a File
     * @example
     * // Get one File
     * const file = await prisma.file.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends FileFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, FileFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'File'> extends True ? CheckSelect<T, Prisma__FileClient<File>, Prisma__FileClient<FileGetPayload<T>>> : CheckSelect<T, Prisma__FileClient<File | null >, Prisma__FileClient<FileGetPayload<T> | null >>

    /**
     * Find zero or more Files that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {FileFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Files
     * const files = await prisma.file.findMany()
     * 
     * // Get first 10 Files
     * const files = await prisma.file.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const fileWithIdOnly = await prisma.file.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends FileFindManyArgs>(
      args?: SelectSubset<T, FileFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<File>>, PrismaPromise<Array<FileGetPayload<T>>>>

    /**
     * Create a File.
     * @param {FileCreateArgs} args - Arguments to create a File.
     * @example
     * // Create one File
     * const File = await prisma.file.create({
     *   data: {
     *     // ... data to create a File
     *   }
     * })
     * 
    **/
    create<T extends FileCreateArgs>(
      args: SelectSubset<T, FileCreateArgs>
    ): CheckSelect<T, Prisma__FileClient<File>, Prisma__FileClient<FileGetPayload<T>>>

    /**
     * Delete a File.
     * @param {FileDeleteArgs} args - Arguments to delete one File.
     * @example
     * // Delete one File
     * const File = await prisma.file.delete({
     *   where: {
     *     // ... filter to delete one File
     *   }
     * })
     * 
    **/
    delete<T extends FileDeleteArgs>(
      args: SelectSubset<T, FileDeleteArgs>
    ): CheckSelect<T, Prisma__FileClient<File>, Prisma__FileClient<FileGetPayload<T>>>

    /**
     * Update one File.
     * @param {FileUpdateArgs} args - Arguments to update one File.
     * @example
     * // Update one File
     * const file = await prisma.file.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends FileUpdateArgs>(
      args: SelectSubset<T, FileUpdateArgs>
    ): CheckSelect<T, Prisma__FileClient<File>, Prisma__FileClient<FileGetPayload<T>>>

    /**
     * Delete zero or more Files.
     * @param {FileDeleteManyArgs} args - Arguments to filter Files to delete.
     * @example
     * // Delete a few Files
     * const { count } = await prisma.file.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends FileDeleteManyArgs>(
      args?: SelectSubset<T, FileDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Files.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {FileUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Files
     * const file = await prisma.file.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends FileUpdateManyArgs>(
      args: SelectSubset<T, FileUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one File.
     * @param {FileUpsertArgs} args - Arguments to update or create a File.
     * @example
     * // Update or create a File
     * const file = await prisma.file.upsert({
     *   create: {
     *     // ... data to create a File
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the File we want to update
     *   }
     * })
    **/
    upsert<T extends FileUpsertArgs>(
      args: SelectSubset<T, FileUpsertArgs>
    ): CheckSelect<T, Prisma__FileClient<File>, Prisma__FileClient<FileGetPayload<T>>>

    /**
     * Count the number of Files.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {FileCountArgs} args - Arguments to filter Files to count.
     * @example
     * // Count the number of Files
     * const count = await prisma.file.count({
     *   where: {
     *     // ... the filter for the Files we want to count
     *   }
     * })
    **/
    count<T extends FileCountArgs>(
      args?: Subset<T, FileCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], FileCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a File.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {FileAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends FileAggregateArgs>(args: Subset<T, FileAggregateArgs>): PrismaPromise<GetFileAggregateType<T>>

    /**
     * Group by File.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {FileGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends FileGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: FileGroupByArgs['orderBy'] }
        : { orderBy?: FileGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, FileGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetFileGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for File.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__FileClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    location<T extends LocationArgs = {}>(args?: Subset<T, LocationArgs>): CheckSelect<T, Prisma__LocationClient<Location | null >, Prisma__LocationClient<LocationGetPayload<T> | null >>;

    parent<T extends FileArgs = {}>(args?: Subset<T, FileArgs>): CheckSelect<T, Prisma__FileClient<File | null >, Prisma__FileClient<FileGetPayload<T> | null >>;

    children<T extends FileFindManyArgs = {}>(args?: Subset<T, FileFindManyArgs>): CheckSelect<T, PrismaPromise<Array<File>>, PrismaPromise<Array<FileGetPayload<T>>>>;

    file_tags<T extends TagOnFileFindManyArgs = {}>(args?: Subset<T, TagOnFileFindManyArgs>): CheckSelect<T, PrismaPromise<Array<TagOnFile>>, PrismaPromise<Array<TagOnFileGetPayload<T>>>>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * File findUnique
   */
  export type FileFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * Throw an Error if a File can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which File to fetch.
     * 
    **/
    where: FileWhereUniqueInput
  }


  /**
   * File findFirst
   */
  export type FileFindFirstArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * Throw an Error if a File can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which File to fetch.
     * 
    **/
    where?: FileWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Files to fetch.
     * 
    **/
    orderBy?: Enumerable<FileOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Files.
     * 
    **/
    cursor?: FileWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Files from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Files.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Files.
     * 
    **/
    distinct?: Enumerable<FileScalarFieldEnum>
  }


  /**
   * File findMany
   */
  export type FileFindManyArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * Filter, which Files to fetch.
     * 
    **/
    where?: FileWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Files to fetch.
     * 
    **/
    orderBy?: Enumerable<FileOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Files.
     * 
    **/
    cursor?: FileWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Files from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Files.
     * 
    **/
    skip?: number
    distinct?: Enumerable<FileScalarFieldEnum>
  }


  /**
   * File create
   */
  export type FileCreateArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * The data needed to create a File.
     * 
    **/
    data: XOR<FileCreateInput, FileUncheckedCreateInput>
  }


  /**
   * File update
   */
  export type FileUpdateArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * The data needed to update a File.
     * 
    **/
    data: XOR<FileUpdateInput, FileUncheckedUpdateInput>
    /**
     * Choose, which File to update.
     * 
    **/
    where: FileWhereUniqueInput
  }


  /**
   * File updateMany
   */
  export type FileUpdateManyArgs = {
    /**
     * The data used to update Files.
     * 
    **/
    data: XOR<FileUpdateManyMutationInput, FileUncheckedUpdateManyInput>
    /**
     * Filter which Files to update
     * 
    **/
    where?: FileWhereInput
  }


  /**
   * File upsert
   */
  export type FileUpsertArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * The filter to search for the File to update in case it exists.
     * 
    **/
    where: FileWhereUniqueInput
    /**
     * In case the File found by the `where` argument doesn't exist, create a new File with this data.
     * 
    **/
    create: XOR<FileCreateInput, FileUncheckedCreateInput>
    /**
     * In case the File was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<FileUpdateInput, FileUncheckedUpdateInput>
  }


  /**
   * File delete
   */
  export type FileDeleteArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
    /**
     * Filter which File to delete.
     * 
    **/
    where: FileWhereUniqueInput
  }


  /**
   * File deleteMany
   */
  export type FileDeleteManyArgs = {
    /**
     * Filter which Files to delete
     * 
    **/
    where?: FileWhereInput
  }


  /**
   * File without action
   */
  export type FileArgs = {
    /**
     * Select specific fields to fetch from the File
     * 
    **/
    select?: FileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: FileInclude | null
  }



  /**
   * Model Tag
   */


  export type AggregateTag = {
    _count: TagCountAggregateOutputType | null
    _avg: TagAvgAggregateOutputType | null
    _sum: TagSumAggregateOutputType | null
    _min: TagMinAggregateOutputType | null
    _max: TagMaxAggregateOutputType | null
  }

  export type TagAvgAggregateOutputType = {
    id: number | null
    encryption: number | null
    total_files: number | null
    redundancy_goal: number | null
  }

  export type TagSumAggregateOutputType = {
    id: number | null
    encryption: number | null
    total_files: number | null
    redundancy_goal: number | null
  }

  export type TagMinAggregateOutputType = {
    id: number | null
    name: string | null
    encryption: number | null
    total_files: number | null
    redundancy_goal: number | null
    date_created: Date | null
    date_modified: Date | null
  }

  export type TagMaxAggregateOutputType = {
    id: number | null
    name: string | null
    encryption: number | null
    total_files: number | null
    redundancy_goal: number | null
    date_created: Date | null
    date_modified: Date | null
  }

  export type TagCountAggregateOutputType = {
    id: number
    name: number
    encryption: number
    total_files: number
    redundancy_goal: number
    date_created: number
    date_modified: number
    _all: number
  }


  export type TagAvgAggregateInputType = {
    id?: true
    encryption?: true
    total_files?: true
    redundancy_goal?: true
  }

  export type TagSumAggregateInputType = {
    id?: true
    encryption?: true
    total_files?: true
    redundancy_goal?: true
  }

  export type TagMinAggregateInputType = {
    id?: true
    name?: true
    encryption?: true
    total_files?: true
    redundancy_goal?: true
    date_created?: true
    date_modified?: true
  }

  export type TagMaxAggregateInputType = {
    id?: true
    name?: true
    encryption?: true
    total_files?: true
    redundancy_goal?: true
    date_created?: true
    date_modified?: true
  }

  export type TagCountAggregateInputType = {
    id?: true
    name?: true
    encryption?: true
    total_files?: true
    redundancy_goal?: true
    date_created?: true
    date_modified?: true
    _all?: true
  }

  export type TagAggregateArgs = {
    /**
     * Filter which Tag to aggregate.
     * 
    **/
    where?: TagWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Tags to fetch.
     * 
    **/
    orderBy?: Enumerable<TagOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: TagWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Tags from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Tags.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Tags
    **/
    _count?: true | TagCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: TagAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: TagSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: TagMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: TagMaxAggregateInputType
  }

  export type GetTagAggregateType<T extends TagAggregateArgs> = {
        [P in keyof T & keyof AggregateTag]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateTag[P]>
      : GetScalarType<T[P], AggregateTag[P]>
  }




  export type TagGroupByArgs = {
    where?: TagWhereInput
    orderBy?: Enumerable<TagOrderByWithAggregationInput>
    by: Array<TagScalarFieldEnum>
    having?: TagScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: TagCountAggregateInputType | true
    _avg?: TagAvgAggregateInputType
    _sum?: TagSumAggregateInputType
    _min?: TagMinAggregateInputType
    _max?: TagMaxAggregateInputType
  }


  export type TagGroupByOutputType = {
    id: number
    name: string | null
    encryption: number | null
    total_files: number | null
    redundancy_goal: number | null
    date_created: Date
    date_modified: Date
    _count: TagCountAggregateOutputType | null
    _avg: TagAvgAggregateOutputType | null
    _sum: TagSumAggregateOutputType | null
    _min: TagMinAggregateOutputType | null
    _max: TagMaxAggregateOutputType | null
  }

  type GetTagGroupByPayload<T extends TagGroupByArgs> = PrismaPromise<
    Array<
      PickArray<TagGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof TagGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], TagGroupByOutputType[P]>
            : GetScalarType<T[P], TagGroupByOutputType[P]>
        }
      >
    >


  export type TagSelect = {
    id?: boolean
    name?: boolean
    encryption?: boolean
    total_files?: boolean
    redundancy_goal?: boolean
    date_created?: boolean
    date_modified?: boolean
    tag_files?: boolean | TagOnFileFindManyArgs
    _count?: boolean | TagCountOutputTypeArgs
  }

  export type TagInclude = {
    tag_files?: boolean | TagOnFileFindManyArgs
    _count?: boolean | TagCountOutputTypeArgs
  }

  export type TagGetPayload<
    S extends boolean | null | undefined | TagArgs,
    U = keyof S
      > = S extends true
        ? Tag
    : S extends undefined
    ? never
    : S extends TagArgs | TagFindManyArgs
    ?'include' extends U
    ? Tag  & {
    [P in TrueKeys<S['include']>]:
        P extends 'tag_files' ? Array < TagOnFileGetPayload<S['include'][P]>>  :
        P extends '_count' ? TagCountOutputTypeGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'tag_files' ? Array < TagOnFileGetPayload<S['select'][P]>>  :
        P extends '_count' ? TagCountOutputTypeGetPayload<S['select'][P]> :  P extends keyof Tag ? Tag[P] : never
  } 
    : Tag
  : Tag


  type TagCountArgs = Merge<
    Omit<TagFindManyArgs, 'select' | 'include'> & {
      select?: TagCountAggregateInputType | true
    }
  >

  export interface TagDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Tag that matches the filter.
     * @param {TagFindUniqueArgs} args - Arguments to find a Tag
     * @example
     * // Get one Tag
     * const tag = await prisma.tag.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends TagFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, TagFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Tag'> extends True ? CheckSelect<T, Prisma__TagClient<Tag>, Prisma__TagClient<TagGetPayload<T>>> : CheckSelect<T, Prisma__TagClient<Tag | null >, Prisma__TagClient<TagGetPayload<T> | null >>

    /**
     * Find the first Tag that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagFindFirstArgs} args - Arguments to find a Tag
     * @example
     * // Get one Tag
     * const tag = await prisma.tag.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends TagFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, TagFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Tag'> extends True ? CheckSelect<T, Prisma__TagClient<Tag>, Prisma__TagClient<TagGetPayload<T>>> : CheckSelect<T, Prisma__TagClient<Tag | null >, Prisma__TagClient<TagGetPayload<T> | null >>

    /**
     * Find zero or more Tags that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Tags
     * const tags = await prisma.tag.findMany()
     * 
     * // Get first 10 Tags
     * const tags = await prisma.tag.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const tagWithIdOnly = await prisma.tag.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends TagFindManyArgs>(
      args?: SelectSubset<T, TagFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Tag>>, PrismaPromise<Array<TagGetPayload<T>>>>

    /**
     * Create a Tag.
     * @param {TagCreateArgs} args - Arguments to create a Tag.
     * @example
     * // Create one Tag
     * const Tag = await prisma.tag.create({
     *   data: {
     *     // ... data to create a Tag
     *   }
     * })
     * 
    **/
    create<T extends TagCreateArgs>(
      args: SelectSubset<T, TagCreateArgs>
    ): CheckSelect<T, Prisma__TagClient<Tag>, Prisma__TagClient<TagGetPayload<T>>>

    /**
     * Delete a Tag.
     * @param {TagDeleteArgs} args - Arguments to delete one Tag.
     * @example
     * // Delete one Tag
     * const Tag = await prisma.tag.delete({
     *   where: {
     *     // ... filter to delete one Tag
     *   }
     * })
     * 
    **/
    delete<T extends TagDeleteArgs>(
      args: SelectSubset<T, TagDeleteArgs>
    ): CheckSelect<T, Prisma__TagClient<Tag>, Prisma__TagClient<TagGetPayload<T>>>

    /**
     * Update one Tag.
     * @param {TagUpdateArgs} args - Arguments to update one Tag.
     * @example
     * // Update one Tag
     * const tag = await prisma.tag.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends TagUpdateArgs>(
      args: SelectSubset<T, TagUpdateArgs>
    ): CheckSelect<T, Prisma__TagClient<Tag>, Prisma__TagClient<TagGetPayload<T>>>

    /**
     * Delete zero or more Tags.
     * @param {TagDeleteManyArgs} args - Arguments to filter Tags to delete.
     * @example
     * // Delete a few Tags
     * const { count } = await prisma.tag.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends TagDeleteManyArgs>(
      args?: SelectSubset<T, TagDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Tags.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Tags
     * const tag = await prisma.tag.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends TagUpdateManyArgs>(
      args: SelectSubset<T, TagUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Tag.
     * @param {TagUpsertArgs} args - Arguments to update or create a Tag.
     * @example
     * // Update or create a Tag
     * const tag = await prisma.tag.upsert({
     *   create: {
     *     // ... data to create a Tag
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Tag we want to update
     *   }
     * })
    **/
    upsert<T extends TagUpsertArgs>(
      args: SelectSubset<T, TagUpsertArgs>
    ): CheckSelect<T, Prisma__TagClient<Tag>, Prisma__TagClient<TagGetPayload<T>>>

    /**
     * Count the number of Tags.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagCountArgs} args - Arguments to filter Tags to count.
     * @example
     * // Count the number of Tags
     * const count = await prisma.tag.count({
     *   where: {
     *     // ... the filter for the Tags we want to count
     *   }
     * })
    **/
    count<T extends TagCountArgs>(
      args?: Subset<T, TagCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], TagCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Tag.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends TagAggregateArgs>(args: Subset<T, TagAggregateArgs>): PrismaPromise<GetTagAggregateType<T>>

    /**
     * Group by Tag.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends TagGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: TagGroupByArgs['orderBy'] }
        : { orderBy?: TagGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, TagGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetTagGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Tag.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__TagClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    tag_files<T extends TagOnFileFindManyArgs = {}>(args?: Subset<T, TagOnFileFindManyArgs>): CheckSelect<T, PrismaPromise<Array<TagOnFile>>, PrismaPromise<Array<TagOnFileGetPayload<T>>>>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Tag findUnique
   */
  export type TagFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * Throw an Error if a Tag can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Tag to fetch.
     * 
    **/
    where: TagWhereUniqueInput
  }


  /**
   * Tag findFirst
   */
  export type TagFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * Throw an Error if a Tag can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Tag to fetch.
     * 
    **/
    where?: TagWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Tags to fetch.
     * 
    **/
    orderBy?: Enumerable<TagOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Tags.
     * 
    **/
    cursor?: TagWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Tags from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Tags.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Tags.
     * 
    **/
    distinct?: Enumerable<TagScalarFieldEnum>
  }


  /**
   * Tag findMany
   */
  export type TagFindManyArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * Filter, which Tags to fetch.
     * 
    **/
    where?: TagWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Tags to fetch.
     * 
    **/
    orderBy?: Enumerable<TagOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Tags.
     * 
    **/
    cursor?: TagWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Tags from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Tags.
     * 
    **/
    skip?: number
    distinct?: Enumerable<TagScalarFieldEnum>
  }


  /**
   * Tag create
   */
  export type TagCreateArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * The data needed to create a Tag.
     * 
    **/
    data: XOR<TagCreateInput, TagUncheckedCreateInput>
  }


  /**
   * Tag update
   */
  export type TagUpdateArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * The data needed to update a Tag.
     * 
    **/
    data: XOR<TagUpdateInput, TagUncheckedUpdateInput>
    /**
     * Choose, which Tag to update.
     * 
    **/
    where: TagWhereUniqueInput
  }


  /**
   * Tag updateMany
   */
  export type TagUpdateManyArgs = {
    /**
     * The data used to update Tags.
     * 
    **/
    data: XOR<TagUpdateManyMutationInput, TagUncheckedUpdateManyInput>
    /**
     * Filter which Tags to update
     * 
    **/
    where?: TagWhereInput
  }


  /**
   * Tag upsert
   */
  export type TagUpsertArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * The filter to search for the Tag to update in case it exists.
     * 
    **/
    where: TagWhereUniqueInput
    /**
     * In case the Tag found by the `where` argument doesn't exist, create a new Tag with this data.
     * 
    **/
    create: XOR<TagCreateInput, TagUncheckedCreateInput>
    /**
     * In case the Tag was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<TagUpdateInput, TagUncheckedUpdateInput>
  }


  /**
   * Tag delete
   */
  export type TagDeleteArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
    /**
     * Filter which Tag to delete.
     * 
    **/
    where: TagWhereUniqueInput
  }


  /**
   * Tag deleteMany
   */
  export type TagDeleteManyArgs = {
    /**
     * Filter which Tags to delete
     * 
    **/
    where?: TagWhereInput
  }


  /**
   * Tag without action
   */
  export type TagArgs = {
    /**
     * Select specific fields to fetch from the Tag
     * 
    **/
    select?: TagSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagInclude | null
  }



  /**
   * Model TagOnFile
   */


  export type AggregateTagOnFile = {
    _count: TagOnFileCountAggregateOutputType | null
    _avg: TagOnFileAvgAggregateOutputType | null
    _sum: TagOnFileSumAggregateOutputType | null
    _min: TagOnFileMinAggregateOutputType | null
    _max: TagOnFileMaxAggregateOutputType | null
  }

  export type TagOnFileAvgAggregateOutputType = {
    tag_id: number | null
    file_id: number | null
  }

  export type TagOnFileSumAggregateOutputType = {
    tag_id: number | null
    file_id: number | null
  }

  export type TagOnFileMinAggregateOutputType = {
    date_created: Date | null
    tag_id: number | null
    file_id: number | null
  }

  export type TagOnFileMaxAggregateOutputType = {
    date_created: Date | null
    tag_id: number | null
    file_id: number | null
  }

  export type TagOnFileCountAggregateOutputType = {
    date_created: number
    tag_id: number
    file_id: number
    _all: number
  }


  export type TagOnFileAvgAggregateInputType = {
    tag_id?: true
    file_id?: true
  }

  export type TagOnFileSumAggregateInputType = {
    tag_id?: true
    file_id?: true
  }

  export type TagOnFileMinAggregateInputType = {
    date_created?: true
    tag_id?: true
    file_id?: true
  }

  export type TagOnFileMaxAggregateInputType = {
    date_created?: true
    tag_id?: true
    file_id?: true
  }

  export type TagOnFileCountAggregateInputType = {
    date_created?: true
    tag_id?: true
    file_id?: true
    _all?: true
  }

  export type TagOnFileAggregateArgs = {
    /**
     * Filter which TagOnFile to aggregate.
     * 
    **/
    where?: TagOnFileWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of TagOnFiles to fetch.
     * 
    **/
    orderBy?: Enumerable<TagOnFileOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: TagOnFileWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` TagOnFiles from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` TagOnFiles.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned TagOnFiles
    **/
    _count?: true | TagOnFileCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: TagOnFileAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: TagOnFileSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: TagOnFileMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: TagOnFileMaxAggregateInputType
  }

  export type GetTagOnFileAggregateType<T extends TagOnFileAggregateArgs> = {
        [P in keyof T & keyof AggregateTagOnFile]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateTagOnFile[P]>
      : GetScalarType<T[P], AggregateTagOnFile[P]>
  }




  export type TagOnFileGroupByArgs = {
    where?: TagOnFileWhereInput
    orderBy?: Enumerable<TagOnFileOrderByWithAggregationInput>
    by: Array<TagOnFileScalarFieldEnum>
    having?: TagOnFileScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: TagOnFileCountAggregateInputType | true
    _avg?: TagOnFileAvgAggregateInputType
    _sum?: TagOnFileSumAggregateInputType
    _min?: TagOnFileMinAggregateInputType
    _max?: TagOnFileMaxAggregateInputType
  }


  export type TagOnFileGroupByOutputType = {
    date_created: Date
    tag_id: number
    file_id: number
    _count: TagOnFileCountAggregateOutputType | null
    _avg: TagOnFileAvgAggregateOutputType | null
    _sum: TagOnFileSumAggregateOutputType | null
    _min: TagOnFileMinAggregateOutputType | null
    _max: TagOnFileMaxAggregateOutputType | null
  }

  type GetTagOnFileGroupByPayload<T extends TagOnFileGroupByArgs> = PrismaPromise<
    Array<
      PickArray<TagOnFileGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof TagOnFileGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], TagOnFileGroupByOutputType[P]>
            : GetScalarType<T[P], TagOnFileGroupByOutputType[P]>
        }
      >
    >


  export type TagOnFileSelect = {
    date_created?: boolean
    tag_id?: boolean
    tag?: boolean | TagArgs
    file_id?: boolean
    file?: boolean | FileArgs
  }

  export type TagOnFileInclude = {
    tag?: boolean | TagArgs
    file?: boolean | FileArgs
  }

  export type TagOnFileGetPayload<
    S extends boolean | null | undefined | TagOnFileArgs,
    U = keyof S
      > = S extends true
        ? TagOnFile
    : S extends undefined
    ? never
    : S extends TagOnFileArgs | TagOnFileFindManyArgs
    ?'include' extends U
    ? TagOnFile  & {
    [P in TrueKeys<S['include']>]:
        P extends 'tag' ? TagGetPayload<S['include'][P]> :
        P extends 'file' ? FileGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'tag' ? TagGetPayload<S['select'][P]> :
        P extends 'file' ? FileGetPayload<S['select'][P]> :  P extends keyof TagOnFile ? TagOnFile[P] : never
  } 
    : TagOnFile
  : TagOnFile


  type TagOnFileCountArgs = Merge<
    Omit<TagOnFileFindManyArgs, 'select' | 'include'> & {
      select?: TagOnFileCountAggregateInputType | true
    }
  >

  export interface TagOnFileDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one TagOnFile that matches the filter.
     * @param {TagOnFileFindUniqueArgs} args - Arguments to find a TagOnFile
     * @example
     * // Get one TagOnFile
     * const tagOnFile = await prisma.tagOnFile.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends TagOnFileFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, TagOnFileFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'TagOnFile'> extends True ? CheckSelect<T, Prisma__TagOnFileClient<TagOnFile>, Prisma__TagOnFileClient<TagOnFileGetPayload<T>>> : CheckSelect<T, Prisma__TagOnFileClient<TagOnFile | null >, Prisma__TagOnFileClient<TagOnFileGetPayload<T> | null >>

    /**
     * Find the first TagOnFile that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagOnFileFindFirstArgs} args - Arguments to find a TagOnFile
     * @example
     * // Get one TagOnFile
     * const tagOnFile = await prisma.tagOnFile.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends TagOnFileFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, TagOnFileFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'TagOnFile'> extends True ? CheckSelect<T, Prisma__TagOnFileClient<TagOnFile>, Prisma__TagOnFileClient<TagOnFileGetPayload<T>>> : CheckSelect<T, Prisma__TagOnFileClient<TagOnFile | null >, Prisma__TagOnFileClient<TagOnFileGetPayload<T> | null >>

    /**
     * Find zero or more TagOnFiles that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagOnFileFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all TagOnFiles
     * const tagOnFiles = await prisma.tagOnFile.findMany()
     * 
     * // Get first 10 TagOnFiles
     * const tagOnFiles = await prisma.tagOnFile.findMany({ take: 10 })
     * 
     * // Only select the `date_created`
     * const tagOnFileWithDate_createdOnly = await prisma.tagOnFile.findMany({ select: { date_created: true } })
     * 
    **/
    findMany<T extends TagOnFileFindManyArgs>(
      args?: SelectSubset<T, TagOnFileFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<TagOnFile>>, PrismaPromise<Array<TagOnFileGetPayload<T>>>>

    /**
     * Create a TagOnFile.
     * @param {TagOnFileCreateArgs} args - Arguments to create a TagOnFile.
     * @example
     * // Create one TagOnFile
     * const TagOnFile = await prisma.tagOnFile.create({
     *   data: {
     *     // ... data to create a TagOnFile
     *   }
     * })
     * 
    **/
    create<T extends TagOnFileCreateArgs>(
      args: SelectSubset<T, TagOnFileCreateArgs>
    ): CheckSelect<T, Prisma__TagOnFileClient<TagOnFile>, Prisma__TagOnFileClient<TagOnFileGetPayload<T>>>

    /**
     * Delete a TagOnFile.
     * @param {TagOnFileDeleteArgs} args - Arguments to delete one TagOnFile.
     * @example
     * // Delete one TagOnFile
     * const TagOnFile = await prisma.tagOnFile.delete({
     *   where: {
     *     // ... filter to delete one TagOnFile
     *   }
     * })
     * 
    **/
    delete<T extends TagOnFileDeleteArgs>(
      args: SelectSubset<T, TagOnFileDeleteArgs>
    ): CheckSelect<T, Prisma__TagOnFileClient<TagOnFile>, Prisma__TagOnFileClient<TagOnFileGetPayload<T>>>

    /**
     * Update one TagOnFile.
     * @param {TagOnFileUpdateArgs} args - Arguments to update one TagOnFile.
     * @example
     * // Update one TagOnFile
     * const tagOnFile = await prisma.tagOnFile.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends TagOnFileUpdateArgs>(
      args: SelectSubset<T, TagOnFileUpdateArgs>
    ): CheckSelect<T, Prisma__TagOnFileClient<TagOnFile>, Prisma__TagOnFileClient<TagOnFileGetPayload<T>>>

    /**
     * Delete zero or more TagOnFiles.
     * @param {TagOnFileDeleteManyArgs} args - Arguments to filter TagOnFiles to delete.
     * @example
     * // Delete a few TagOnFiles
     * const { count } = await prisma.tagOnFile.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends TagOnFileDeleteManyArgs>(
      args?: SelectSubset<T, TagOnFileDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more TagOnFiles.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagOnFileUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many TagOnFiles
     * const tagOnFile = await prisma.tagOnFile.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends TagOnFileUpdateManyArgs>(
      args: SelectSubset<T, TagOnFileUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one TagOnFile.
     * @param {TagOnFileUpsertArgs} args - Arguments to update or create a TagOnFile.
     * @example
     * // Update or create a TagOnFile
     * const tagOnFile = await prisma.tagOnFile.upsert({
     *   create: {
     *     // ... data to create a TagOnFile
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the TagOnFile we want to update
     *   }
     * })
    **/
    upsert<T extends TagOnFileUpsertArgs>(
      args: SelectSubset<T, TagOnFileUpsertArgs>
    ): CheckSelect<T, Prisma__TagOnFileClient<TagOnFile>, Prisma__TagOnFileClient<TagOnFileGetPayload<T>>>

    /**
     * Count the number of TagOnFiles.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagOnFileCountArgs} args - Arguments to filter TagOnFiles to count.
     * @example
     * // Count the number of TagOnFiles
     * const count = await prisma.tagOnFile.count({
     *   where: {
     *     // ... the filter for the TagOnFiles we want to count
     *   }
     * })
    **/
    count<T extends TagOnFileCountArgs>(
      args?: Subset<T, TagOnFileCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], TagOnFileCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a TagOnFile.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagOnFileAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends TagOnFileAggregateArgs>(args: Subset<T, TagOnFileAggregateArgs>): PrismaPromise<GetTagOnFileAggregateType<T>>

    /**
     * Group by TagOnFile.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {TagOnFileGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends TagOnFileGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: TagOnFileGroupByArgs['orderBy'] }
        : { orderBy?: TagOnFileGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, TagOnFileGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetTagOnFileGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for TagOnFile.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__TagOnFileClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    tag<T extends TagArgs = {}>(args?: Subset<T, TagArgs>): CheckSelect<T, Prisma__TagClient<Tag | null >, Prisma__TagClient<TagGetPayload<T> | null >>;

    file<T extends FileArgs = {}>(args?: Subset<T, FileArgs>): CheckSelect<T, Prisma__FileClient<File | null >, Prisma__FileClient<FileGetPayload<T> | null >>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * TagOnFile findUnique
   */
  export type TagOnFileFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * Throw an Error if a TagOnFile can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which TagOnFile to fetch.
     * 
    **/
    where: TagOnFileWhereUniqueInput
  }


  /**
   * TagOnFile findFirst
   */
  export type TagOnFileFindFirstArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * Throw an Error if a TagOnFile can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which TagOnFile to fetch.
     * 
    **/
    where?: TagOnFileWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of TagOnFiles to fetch.
     * 
    **/
    orderBy?: Enumerable<TagOnFileOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for TagOnFiles.
     * 
    **/
    cursor?: TagOnFileWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` TagOnFiles from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` TagOnFiles.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of TagOnFiles.
     * 
    **/
    distinct?: Enumerable<TagOnFileScalarFieldEnum>
  }


  /**
   * TagOnFile findMany
   */
  export type TagOnFileFindManyArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * Filter, which TagOnFiles to fetch.
     * 
    **/
    where?: TagOnFileWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of TagOnFiles to fetch.
     * 
    **/
    orderBy?: Enumerable<TagOnFileOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing TagOnFiles.
     * 
    **/
    cursor?: TagOnFileWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` TagOnFiles from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` TagOnFiles.
     * 
    **/
    skip?: number
    distinct?: Enumerable<TagOnFileScalarFieldEnum>
  }


  /**
   * TagOnFile create
   */
  export type TagOnFileCreateArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * The data needed to create a TagOnFile.
     * 
    **/
    data: XOR<TagOnFileCreateInput, TagOnFileUncheckedCreateInput>
  }


  /**
   * TagOnFile update
   */
  export type TagOnFileUpdateArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * The data needed to update a TagOnFile.
     * 
    **/
    data: XOR<TagOnFileUpdateInput, TagOnFileUncheckedUpdateInput>
    /**
     * Choose, which TagOnFile to update.
     * 
    **/
    where: TagOnFileWhereUniqueInput
  }


  /**
   * TagOnFile updateMany
   */
  export type TagOnFileUpdateManyArgs = {
    /**
     * The data used to update TagOnFiles.
     * 
    **/
    data: XOR<TagOnFileUpdateManyMutationInput, TagOnFileUncheckedUpdateManyInput>
    /**
     * Filter which TagOnFiles to update
     * 
    **/
    where?: TagOnFileWhereInput
  }


  /**
   * TagOnFile upsert
   */
  export type TagOnFileUpsertArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * The filter to search for the TagOnFile to update in case it exists.
     * 
    **/
    where: TagOnFileWhereUniqueInput
    /**
     * In case the TagOnFile found by the `where` argument doesn't exist, create a new TagOnFile with this data.
     * 
    **/
    create: XOR<TagOnFileCreateInput, TagOnFileUncheckedCreateInput>
    /**
     * In case the TagOnFile was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<TagOnFileUpdateInput, TagOnFileUncheckedUpdateInput>
  }


  /**
   * TagOnFile delete
   */
  export type TagOnFileDeleteArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
    /**
     * Filter which TagOnFile to delete.
     * 
    **/
    where: TagOnFileWhereUniqueInput
  }


  /**
   * TagOnFile deleteMany
   */
  export type TagOnFileDeleteManyArgs = {
    /**
     * Filter which TagOnFiles to delete
     * 
    **/
    where?: TagOnFileWhereInput
  }


  /**
   * TagOnFile without action
   */
  export type TagOnFileArgs = {
    /**
     * Select specific fields to fetch from the TagOnFile
     * 
    **/
    select?: TagOnFileSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: TagOnFileInclude | null
  }



  /**
   * Model Job
   */


  export type AggregateJob = {
    _count: JobCountAggregateOutputType | null
    _avg: JobAvgAggregateOutputType | null
    _sum: JobSumAggregateOutputType | null
    _min: JobMinAggregateOutputType | null
    _max: JobMaxAggregateOutputType | null
  }

  export type JobAvgAggregateOutputType = {
    id: number | null
    client_id: number | null
    action: number | null
    status: number | null
    percentage_complete: number | null
    task_count: number | null
    completed_task_count: number | null
  }

  export type JobSumAggregateOutputType = {
    id: number | null
    client_id: number | null
    action: number | null
    status: number | null
    percentage_complete: number | null
    task_count: number | null
    completed_task_count: number | null
  }

  export type JobMinAggregateOutputType = {
    id: number | null
    client_id: number | null
    action: number | null
    status: number | null
    percentage_complete: number | null
    task_count: number | null
    completed_task_count: number | null
    date_created: Date | null
    date_modified: Date | null
  }

  export type JobMaxAggregateOutputType = {
    id: number | null
    client_id: number | null
    action: number | null
    status: number | null
    percentage_complete: number | null
    task_count: number | null
    completed_task_count: number | null
    date_created: Date | null
    date_modified: Date | null
  }

  export type JobCountAggregateOutputType = {
    id: number
    client_id: number
    action: number
    status: number
    percentage_complete: number
    task_count: number
    completed_task_count: number
    date_created: number
    date_modified: number
    _all: number
  }


  export type JobAvgAggregateInputType = {
    id?: true
    client_id?: true
    action?: true
    status?: true
    percentage_complete?: true
    task_count?: true
    completed_task_count?: true
  }

  export type JobSumAggregateInputType = {
    id?: true
    client_id?: true
    action?: true
    status?: true
    percentage_complete?: true
    task_count?: true
    completed_task_count?: true
  }

  export type JobMinAggregateInputType = {
    id?: true
    client_id?: true
    action?: true
    status?: true
    percentage_complete?: true
    task_count?: true
    completed_task_count?: true
    date_created?: true
    date_modified?: true
  }

  export type JobMaxAggregateInputType = {
    id?: true
    client_id?: true
    action?: true
    status?: true
    percentage_complete?: true
    task_count?: true
    completed_task_count?: true
    date_created?: true
    date_modified?: true
  }

  export type JobCountAggregateInputType = {
    id?: true
    client_id?: true
    action?: true
    status?: true
    percentage_complete?: true
    task_count?: true
    completed_task_count?: true
    date_created?: true
    date_modified?: true
    _all?: true
  }

  export type JobAggregateArgs = {
    /**
     * Filter which Job to aggregate.
     * 
    **/
    where?: JobWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Jobs to fetch.
     * 
    **/
    orderBy?: Enumerable<JobOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: JobWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Jobs from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Jobs.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Jobs
    **/
    _count?: true | JobCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: JobAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: JobSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: JobMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: JobMaxAggregateInputType
  }

  export type GetJobAggregateType<T extends JobAggregateArgs> = {
        [P in keyof T & keyof AggregateJob]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateJob[P]>
      : GetScalarType<T[P], AggregateJob[P]>
  }




  export type JobGroupByArgs = {
    where?: JobWhereInput
    orderBy?: Enumerable<JobOrderByWithAggregationInput>
    by: Array<JobScalarFieldEnum>
    having?: JobScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: JobCountAggregateInputType | true
    _avg?: JobAvgAggregateInputType
    _sum?: JobSumAggregateInputType
    _min?: JobMinAggregateInputType
    _max?: JobMaxAggregateInputType
  }


  export type JobGroupByOutputType = {
    id: number
    client_id: number
    action: number
    status: number
    percentage_complete: number
    task_count: number
    completed_task_count: number
    date_created: Date
    date_modified: Date
    _count: JobCountAggregateOutputType | null
    _avg: JobAvgAggregateOutputType | null
    _sum: JobSumAggregateOutputType | null
    _min: JobMinAggregateOutputType | null
    _max: JobMaxAggregateOutputType | null
  }

  type GetJobGroupByPayload<T extends JobGroupByArgs> = PrismaPromise<
    Array<
      PickArray<JobGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof JobGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], JobGroupByOutputType[P]>
            : GetScalarType<T[P], JobGroupByOutputType[P]>
        }
      >
    >


  export type JobSelect = {
    id?: boolean
    client_id?: boolean
    action?: boolean
    status?: boolean
    percentage_complete?: boolean
    task_count?: boolean
    completed_task_count?: boolean
    date_created?: boolean
    date_modified?: boolean
    clients?: boolean | ClientArgs
  }

  export type JobInclude = {
    clients?: boolean | ClientArgs
  }

  export type JobGetPayload<
    S extends boolean | null | undefined | JobArgs,
    U = keyof S
      > = S extends true
        ? Job
    : S extends undefined
    ? never
    : S extends JobArgs | JobFindManyArgs
    ?'include' extends U
    ? Job  & {
    [P in TrueKeys<S['include']>]:
        P extends 'clients' ? ClientGetPayload<S['include'][P]> :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'clients' ? ClientGetPayload<S['select'][P]> :  P extends keyof Job ? Job[P] : never
  } 
    : Job
  : Job


  type JobCountArgs = Merge<
    Omit<JobFindManyArgs, 'select' | 'include'> & {
      select?: JobCountAggregateInputType | true
    }
  >

  export interface JobDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Job that matches the filter.
     * @param {JobFindUniqueArgs} args - Arguments to find a Job
     * @example
     * // Get one Job
     * const job = await prisma.job.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends JobFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, JobFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Job'> extends True ? CheckSelect<T, Prisma__JobClient<Job>, Prisma__JobClient<JobGetPayload<T>>> : CheckSelect<T, Prisma__JobClient<Job | null >, Prisma__JobClient<JobGetPayload<T> | null >>

    /**
     * Find the first Job that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {JobFindFirstArgs} args - Arguments to find a Job
     * @example
     * // Get one Job
     * const job = await prisma.job.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends JobFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, JobFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Job'> extends True ? CheckSelect<T, Prisma__JobClient<Job>, Prisma__JobClient<JobGetPayload<T>>> : CheckSelect<T, Prisma__JobClient<Job | null >, Prisma__JobClient<JobGetPayload<T> | null >>

    /**
     * Find zero or more Jobs that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {JobFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Jobs
     * const jobs = await prisma.job.findMany()
     * 
     * // Get first 10 Jobs
     * const jobs = await prisma.job.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const jobWithIdOnly = await prisma.job.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends JobFindManyArgs>(
      args?: SelectSubset<T, JobFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Job>>, PrismaPromise<Array<JobGetPayload<T>>>>

    /**
     * Create a Job.
     * @param {JobCreateArgs} args - Arguments to create a Job.
     * @example
     * // Create one Job
     * const Job = await prisma.job.create({
     *   data: {
     *     // ... data to create a Job
     *   }
     * })
     * 
    **/
    create<T extends JobCreateArgs>(
      args: SelectSubset<T, JobCreateArgs>
    ): CheckSelect<T, Prisma__JobClient<Job>, Prisma__JobClient<JobGetPayload<T>>>

    /**
     * Delete a Job.
     * @param {JobDeleteArgs} args - Arguments to delete one Job.
     * @example
     * // Delete one Job
     * const Job = await prisma.job.delete({
     *   where: {
     *     // ... filter to delete one Job
     *   }
     * })
     * 
    **/
    delete<T extends JobDeleteArgs>(
      args: SelectSubset<T, JobDeleteArgs>
    ): CheckSelect<T, Prisma__JobClient<Job>, Prisma__JobClient<JobGetPayload<T>>>

    /**
     * Update one Job.
     * @param {JobUpdateArgs} args - Arguments to update one Job.
     * @example
     * // Update one Job
     * const job = await prisma.job.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends JobUpdateArgs>(
      args: SelectSubset<T, JobUpdateArgs>
    ): CheckSelect<T, Prisma__JobClient<Job>, Prisma__JobClient<JobGetPayload<T>>>

    /**
     * Delete zero or more Jobs.
     * @param {JobDeleteManyArgs} args - Arguments to filter Jobs to delete.
     * @example
     * // Delete a few Jobs
     * const { count } = await prisma.job.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends JobDeleteManyArgs>(
      args?: SelectSubset<T, JobDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Jobs.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {JobUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Jobs
     * const job = await prisma.job.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends JobUpdateManyArgs>(
      args: SelectSubset<T, JobUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Job.
     * @param {JobUpsertArgs} args - Arguments to update or create a Job.
     * @example
     * // Update or create a Job
     * const job = await prisma.job.upsert({
     *   create: {
     *     // ... data to create a Job
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Job we want to update
     *   }
     * })
    **/
    upsert<T extends JobUpsertArgs>(
      args: SelectSubset<T, JobUpsertArgs>
    ): CheckSelect<T, Prisma__JobClient<Job>, Prisma__JobClient<JobGetPayload<T>>>

    /**
     * Count the number of Jobs.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {JobCountArgs} args - Arguments to filter Jobs to count.
     * @example
     * // Count the number of Jobs
     * const count = await prisma.job.count({
     *   where: {
     *     // ... the filter for the Jobs we want to count
     *   }
     * })
    **/
    count<T extends JobCountArgs>(
      args?: Subset<T, JobCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], JobCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Job.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {JobAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends JobAggregateArgs>(args: Subset<T, JobAggregateArgs>): PrismaPromise<GetJobAggregateType<T>>

    /**
     * Group by Job.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {JobGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends JobGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: JobGroupByArgs['orderBy'] }
        : { orderBy?: JobGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, JobGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetJobGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Job.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__JobClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    clients<T extends ClientArgs = {}>(args?: Subset<T, ClientArgs>): CheckSelect<T, Prisma__ClientClient<Client | null >, Prisma__ClientClient<ClientGetPayload<T> | null >>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Job findUnique
   */
  export type JobFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * Throw an Error if a Job can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Job to fetch.
     * 
    **/
    where: JobWhereUniqueInput
  }


  /**
   * Job findFirst
   */
  export type JobFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * Throw an Error if a Job can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Job to fetch.
     * 
    **/
    where?: JobWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Jobs to fetch.
     * 
    **/
    orderBy?: Enumerable<JobOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Jobs.
     * 
    **/
    cursor?: JobWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Jobs from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Jobs.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Jobs.
     * 
    **/
    distinct?: Enumerable<JobScalarFieldEnum>
  }


  /**
   * Job findMany
   */
  export type JobFindManyArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * Filter, which Jobs to fetch.
     * 
    **/
    where?: JobWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Jobs to fetch.
     * 
    **/
    orderBy?: Enumerable<JobOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Jobs.
     * 
    **/
    cursor?: JobWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Jobs from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Jobs.
     * 
    **/
    skip?: number
    distinct?: Enumerable<JobScalarFieldEnum>
  }


  /**
   * Job create
   */
  export type JobCreateArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * The data needed to create a Job.
     * 
    **/
    data: XOR<JobCreateInput, JobUncheckedCreateInput>
  }


  /**
   * Job update
   */
  export type JobUpdateArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * The data needed to update a Job.
     * 
    **/
    data: XOR<JobUpdateInput, JobUncheckedUpdateInput>
    /**
     * Choose, which Job to update.
     * 
    **/
    where: JobWhereUniqueInput
  }


  /**
   * Job updateMany
   */
  export type JobUpdateManyArgs = {
    /**
     * The data used to update Jobs.
     * 
    **/
    data: XOR<JobUpdateManyMutationInput, JobUncheckedUpdateManyInput>
    /**
     * Filter which Jobs to update
     * 
    **/
    where?: JobWhereInput
  }


  /**
   * Job upsert
   */
  export type JobUpsertArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * The filter to search for the Job to update in case it exists.
     * 
    **/
    where: JobWhereUniqueInput
    /**
     * In case the Job found by the `where` argument doesn't exist, create a new Job with this data.
     * 
    **/
    create: XOR<JobCreateInput, JobUncheckedCreateInput>
    /**
     * In case the Job was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<JobUpdateInput, JobUncheckedUpdateInput>
  }


  /**
   * Job delete
   */
  export type JobDeleteArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
    /**
     * Filter which Job to delete.
     * 
    **/
    where: JobWhereUniqueInput
  }


  /**
   * Job deleteMany
   */
  export type JobDeleteManyArgs = {
    /**
     * Filter which Jobs to delete
     * 
    **/
    where?: JobWhereInput
  }


  /**
   * Job without action
   */
  export type JobArgs = {
    /**
     * Select specific fields to fetch from the Job
     * 
    **/
    select?: JobSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: JobInclude | null
  }



  /**
   * Model Space
   */


  export type AggregateSpace = {
    _count: SpaceCountAggregateOutputType | null
    _avg: SpaceAvgAggregateOutputType | null
    _sum: SpaceSumAggregateOutputType | null
    _min: SpaceMinAggregateOutputType | null
    _max: SpaceMaxAggregateOutputType | null
  }

  export type SpaceAvgAggregateOutputType = {
    id: number | null
    encryption: number | null
    libraryId: number | null
  }

  export type SpaceSumAggregateOutputType = {
    id: number | null
    encryption: number | null
    libraryId: number | null
  }

  export type SpaceMinAggregateOutputType = {
    id: number | null
    name: string | null
    encryption: number | null
    date_created: Date | null
    date_modified: Date | null
    libraryId: number | null
  }

  export type SpaceMaxAggregateOutputType = {
    id: number | null
    name: string | null
    encryption: number | null
    date_created: Date | null
    date_modified: Date | null
    libraryId: number | null
  }

  export type SpaceCountAggregateOutputType = {
    id: number
    name: number
    encryption: number
    date_created: number
    date_modified: number
    libraryId: number
    _all: number
  }


  export type SpaceAvgAggregateInputType = {
    id?: true
    encryption?: true
    libraryId?: true
  }

  export type SpaceSumAggregateInputType = {
    id?: true
    encryption?: true
    libraryId?: true
  }

  export type SpaceMinAggregateInputType = {
    id?: true
    name?: true
    encryption?: true
    date_created?: true
    date_modified?: true
    libraryId?: true
  }

  export type SpaceMaxAggregateInputType = {
    id?: true
    name?: true
    encryption?: true
    date_created?: true
    date_modified?: true
    libraryId?: true
  }

  export type SpaceCountAggregateInputType = {
    id?: true
    name?: true
    encryption?: true
    date_created?: true
    date_modified?: true
    libraryId?: true
    _all?: true
  }

  export type SpaceAggregateArgs = {
    /**
     * Filter which Space to aggregate.
     * 
    **/
    where?: SpaceWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Spaces to fetch.
     * 
    **/
    orderBy?: Enumerable<SpaceOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the start position
     * 
    **/
    cursor?: SpaceWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Spaces from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Spaces.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Count returned Spaces
    **/
    _count?: true | SpaceCountAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to average
    **/
    _avg?: SpaceAvgAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to sum
    **/
    _sum?: SpaceSumAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the minimum value
    **/
    _min?: SpaceMinAggregateInputType
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/aggregations Aggregation Docs}
     * 
     * Select which fields to find the maximum value
    **/
    _max?: SpaceMaxAggregateInputType
  }

  export type GetSpaceAggregateType<T extends SpaceAggregateArgs> = {
        [P in keyof T & keyof AggregateSpace]: P extends '_count' | 'count'
      ? T[P] extends true
        ? number
        : GetScalarType<T[P], AggregateSpace[P]>
      : GetScalarType<T[P], AggregateSpace[P]>
  }




  export type SpaceGroupByArgs = {
    where?: SpaceWhereInput
    orderBy?: Enumerable<SpaceOrderByWithAggregationInput>
    by: Array<SpaceScalarFieldEnum>
    having?: SpaceScalarWhereWithAggregatesInput
    take?: number
    skip?: number
    _count?: SpaceCountAggregateInputType | true
    _avg?: SpaceAvgAggregateInputType
    _sum?: SpaceSumAggregateInputType
    _min?: SpaceMinAggregateInputType
    _max?: SpaceMaxAggregateInputType
  }


  export type SpaceGroupByOutputType = {
    id: number
    name: string
    encryption: number | null
    date_created: Date
    date_modified: Date
    libraryId: number | null
    _count: SpaceCountAggregateOutputType | null
    _avg: SpaceAvgAggregateOutputType | null
    _sum: SpaceSumAggregateOutputType | null
    _min: SpaceMinAggregateOutputType | null
    _max: SpaceMaxAggregateOutputType | null
  }

  type GetSpaceGroupByPayload<T extends SpaceGroupByArgs> = PrismaPromise<
    Array<
      PickArray<SpaceGroupByOutputType, T['by']> &
        {
          [P in ((keyof T) & (keyof SpaceGroupByOutputType))]: P extends '_count'
            ? T[P] extends boolean
              ? number
              : GetScalarType<T[P], SpaceGroupByOutputType[P]>
            : GetScalarType<T[P], SpaceGroupByOutputType[P]>
        }
      >
    >


  export type SpaceSelect = {
    id?: boolean
    name?: boolean
    encryption?: boolean
    date_created?: boolean
    date_modified?: boolean
    Library?: boolean | LibraryArgs
    libraryId?: boolean
  }

  export type SpaceInclude = {
    Library?: boolean | LibraryArgs
  }

  export type SpaceGetPayload<
    S extends boolean | null | undefined | SpaceArgs,
    U = keyof S
      > = S extends true
        ? Space
    : S extends undefined
    ? never
    : S extends SpaceArgs | SpaceFindManyArgs
    ?'include' extends U
    ? Space  & {
    [P in TrueKeys<S['include']>]:
        P extends 'Library' ? LibraryGetPayload<S['include'][P]> | null :  never
  } 
    : 'select' extends U
    ? {
    [P in TrueKeys<S['select']>]:
        P extends 'Library' ? LibraryGetPayload<S['select'][P]> | null :  P extends keyof Space ? Space[P] : never
  } 
    : Space
  : Space


  type SpaceCountArgs = Merge<
    Omit<SpaceFindManyArgs, 'select' | 'include'> & {
      select?: SpaceCountAggregateInputType | true
    }
  >

  export interface SpaceDelegate<GlobalRejectSettings> {
    /**
     * Find zero or one Space that matches the filter.
     * @param {SpaceFindUniqueArgs} args - Arguments to find a Space
     * @example
     * // Get one Space
     * const space = await prisma.space.findUnique({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findUnique<T extends SpaceFindUniqueArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args: SelectSubset<T, SpaceFindUniqueArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findUnique', 'Space'> extends True ? CheckSelect<T, Prisma__SpaceClient<Space>, Prisma__SpaceClient<SpaceGetPayload<T>>> : CheckSelect<T, Prisma__SpaceClient<Space | null >, Prisma__SpaceClient<SpaceGetPayload<T> | null >>

    /**
     * Find the first Space that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {SpaceFindFirstArgs} args - Arguments to find a Space
     * @example
     * // Get one Space
     * const space = await prisma.space.findFirst({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
    **/
    findFirst<T extends SpaceFindFirstArgs,  LocalRejectSettings = T["rejectOnNotFound"] extends RejectOnNotFound ? T['rejectOnNotFound'] : undefined>(
      args?: SelectSubset<T, SpaceFindFirstArgs>
    ): HasReject<GlobalRejectSettings, LocalRejectSettings, 'findFirst', 'Space'> extends True ? CheckSelect<T, Prisma__SpaceClient<Space>, Prisma__SpaceClient<SpaceGetPayload<T>>> : CheckSelect<T, Prisma__SpaceClient<Space | null >, Prisma__SpaceClient<SpaceGetPayload<T> | null >>

    /**
     * Find zero or more Spaces that matches the filter.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {SpaceFindManyArgs=} args - Arguments to filter and select certain fields only.
     * @example
     * // Get all Spaces
     * const spaces = await prisma.space.findMany()
     * 
     * // Get first 10 Spaces
     * const spaces = await prisma.space.findMany({ take: 10 })
     * 
     * // Only select the `id`
     * const spaceWithIdOnly = await prisma.space.findMany({ select: { id: true } })
     * 
    **/
    findMany<T extends SpaceFindManyArgs>(
      args?: SelectSubset<T, SpaceFindManyArgs>
    ): CheckSelect<T, PrismaPromise<Array<Space>>, PrismaPromise<Array<SpaceGetPayload<T>>>>

    /**
     * Create a Space.
     * @param {SpaceCreateArgs} args - Arguments to create a Space.
     * @example
     * // Create one Space
     * const Space = await prisma.space.create({
     *   data: {
     *     // ... data to create a Space
     *   }
     * })
     * 
    **/
    create<T extends SpaceCreateArgs>(
      args: SelectSubset<T, SpaceCreateArgs>
    ): CheckSelect<T, Prisma__SpaceClient<Space>, Prisma__SpaceClient<SpaceGetPayload<T>>>

    /**
     * Delete a Space.
     * @param {SpaceDeleteArgs} args - Arguments to delete one Space.
     * @example
     * // Delete one Space
     * const Space = await prisma.space.delete({
     *   where: {
     *     // ... filter to delete one Space
     *   }
     * })
     * 
    **/
    delete<T extends SpaceDeleteArgs>(
      args: SelectSubset<T, SpaceDeleteArgs>
    ): CheckSelect<T, Prisma__SpaceClient<Space>, Prisma__SpaceClient<SpaceGetPayload<T>>>

    /**
     * Update one Space.
     * @param {SpaceUpdateArgs} args - Arguments to update one Space.
     * @example
     * // Update one Space
     * const space = await prisma.space.update({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    update<T extends SpaceUpdateArgs>(
      args: SelectSubset<T, SpaceUpdateArgs>
    ): CheckSelect<T, Prisma__SpaceClient<Space>, Prisma__SpaceClient<SpaceGetPayload<T>>>

    /**
     * Delete zero or more Spaces.
     * @param {SpaceDeleteManyArgs} args - Arguments to filter Spaces to delete.
     * @example
     * // Delete a few Spaces
     * const { count } = await prisma.space.deleteMany({
     *   where: {
     *     // ... provide filter here
     *   }
     * })
     * 
    **/
    deleteMany<T extends SpaceDeleteManyArgs>(
      args?: SelectSubset<T, SpaceDeleteManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Update zero or more Spaces.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {SpaceUpdateManyArgs} args - Arguments to update one or more rows.
     * @example
     * // Update many Spaces
     * const space = await prisma.space.updateMany({
     *   where: {
     *     // ... provide filter here
     *   },
     *   data: {
     *     // ... provide data here
     *   }
     * })
     * 
    **/
    updateMany<T extends SpaceUpdateManyArgs>(
      args: SelectSubset<T, SpaceUpdateManyArgs>
    ): PrismaPromise<BatchPayload>

    /**
     * Create or update one Space.
     * @param {SpaceUpsertArgs} args - Arguments to update or create a Space.
     * @example
     * // Update or create a Space
     * const space = await prisma.space.upsert({
     *   create: {
     *     // ... data to create a Space
     *   },
     *   update: {
     *     // ... in case it already exists, update
     *   },
     *   where: {
     *     // ... the filter for the Space we want to update
     *   }
     * })
    **/
    upsert<T extends SpaceUpsertArgs>(
      args: SelectSubset<T, SpaceUpsertArgs>
    ): CheckSelect<T, Prisma__SpaceClient<Space>, Prisma__SpaceClient<SpaceGetPayload<T>>>

    /**
     * Count the number of Spaces.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {SpaceCountArgs} args - Arguments to filter Spaces to count.
     * @example
     * // Count the number of Spaces
     * const count = await prisma.space.count({
     *   where: {
     *     // ... the filter for the Spaces we want to count
     *   }
     * })
    **/
    count<T extends SpaceCountArgs>(
      args?: Subset<T, SpaceCountArgs>,
    ): PrismaPromise<
      T extends _Record<'select', any>
        ? T['select'] extends true
          ? number
          : GetScalarType<T['select'], SpaceCountAggregateOutputType>
        : number
    >

    /**
     * Allows you to perform aggregations operations on a Space.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {SpaceAggregateArgs} args - Select which aggregations you would like to apply and on what fields.
     * @example
     * // Ordered by age ascending
     * // Where email contains prisma.io
     * // Limited to the 10 users
     * const aggregations = await prisma.user.aggregate({
     *   _avg: {
     *     age: true,
     *   },
     *   where: {
     *     email: {
     *       contains: "prisma.io",
     *     },
     *   },
     *   orderBy: {
     *     age: "asc",
     *   },
     *   take: 10,
     * })
    **/
    aggregate<T extends SpaceAggregateArgs>(args: Subset<T, SpaceAggregateArgs>): PrismaPromise<GetSpaceAggregateType<T>>

    /**
     * Group by Space.
     * Note, that providing `undefined` is treated as the value not being there.
     * Read more here: https://pris.ly/d/null-undefined
     * @param {SpaceGroupByArgs} args - Group by arguments.
     * @example
     * // Group by city, order by createdAt, get count
     * const result = await prisma.user.groupBy({
     *   by: ['city', 'createdAt'],
     *   orderBy: {
     *     createdAt: true
     *   },
     *   _count: {
     *     _all: true
     *   },
     * })
     * 
    **/
    groupBy<
      T extends SpaceGroupByArgs,
      HasSelectOrTake extends Or<
        Extends<'skip', Keys<T>>,
        Extends<'take', Keys<T>>
      >,
      OrderByArg extends True extends HasSelectOrTake
        ? { orderBy: SpaceGroupByArgs['orderBy'] }
        : { orderBy?: SpaceGroupByArgs['orderBy'] },
      OrderFields extends ExcludeUnderscoreKeys<Keys<MaybeTupleToUnion<T['orderBy']>>>,
      ByFields extends TupleToUnion<T['by']>,
      ByValid extends Has<ByFields, OrderFields>,
      HavingFields extends GetHavingFields<T['having']>,
      HavingValid extends Has<ByFields, HavingFields>,
      ByEmpty extends T['by'] extends never[] ? True : False,
      InputErrors extends ByEmpty extends True
      ? `Error: "by" must not be empty.`
      : HavingValid extends False
      ? {
          [P in HavingFields]: P extends ByFields
            ? never
            : P extends string
            ? `Error: Field "${P}" used in "having" needs to be provided in "by".`
            : [
                Error,
                'Field ',
                P,
                ` in "having" needs to be provided in "by"`,
              ]
        }[HavingFields]
      : 'take' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "take", you also need to provide "orderBy"'
      : 'skip' extends Keys<T>
      ? 'orderBy' extends Keys<T>
        ? ByValid extends True
          ? {}
          : {
              [P in OrderFields]: P extends ByFields
                ? never
                : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
            }[OrderFields]
        : 'Error: If you provide "skip", you also need to provide "orderBy"'
      : ByValid extends True
      ? {}
      : {
          [P in OrderFields]: P extends ByFields
            ? never
            : `Error: Field "${P}" in "orderBy" needs to be provided in "by"`
        }[OrderFields]
    >(args: SubsetIntersection<T, SpaceGroupByArgs, OrderByArg> & InputErrors): {} extends InputErrors ? GetSpaceGroupByPayload<T> : PrismaPromise<InputErrors>
  }

  /**
   * The delegate class that acts as a "Promise-like" for Space.
   * Why is this prefixed with `Prisma__`?
   * Because we want to prevent naming conflicts as mentioned in
   * https://github.com/prisma/prisma-client-js/issues/707
   */
  export class Prisma__SpaceClient<T> implements PrismaPromise<T> {
    [prisma]: true;
    private readonly _dmmf;
    private readonly _fetcher;
    private readonly _queryType;
    private readonly _rootField;
    private readonly _clientMethod;
    private readonly _args;
    private readonly _dataPath;
    private readonly _errorFormat;
    private readonly _measurePerformance?;
    private _isList;
    private _callsite;
    private _requestPromise?;
    constructor(_dmmf: runtime.DMMFClass, _fetcher: PrismaClientFetcher, _queryType: 'query' | 'mutation', _rootField: string, _clientMethod: string, _args: any, _dataPath: string[], _errorFormat: ErrorFormat, _measurePerformance?: boolean | undefined, _isList?: boolean);
    readonly [Symbol.toStringTag]: 'PrismaClientPromise';

    Library<T extends LibraryArgs = {}>(args?: Subset<T, LibraryArgs>): CheckSelect<T, Prisma__LibraryClient<Library | null >, Prisma__LibraryClient<LibraryGetPayload<T> | null >>;

    private get _document();
    /**
     * Attaches callbacks for the resolution and/or rejection of the Promise.
     * @param onfulfilled The callback to execute when the Promise is resolved.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of which ever callback is executed.
     */
    then<TResult1 = T, TResult2 = never>(onfulfilled?: ((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null, onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | undefined | null): Promise<TResult1 | TResult2>;
    /**
     * Attaches a callback for only the rejection of the Promise.
     * @param onrejected The callback to execute when the Promise is rejected.
     * @returns A Promise for the completion of the callback.
     */
    catch<TResult = never>(onrejected?: ((reason: any) => TResult | PromiseLike<TResult>) | undefined | null): Promise<T | TResult>;
    /**
     * Attaches a callback that is invoked when the Promise is settled (fulfilled or rejected). The
     * resolved value cannot be modified from the callback.
     * @param onfinally The callback to execute when the Promise is settled (fulfilled or rejected).
     * @returns A Promise for the completion of the callback.
     */
    finally(onfinally?: (() => void) | undefined | null): Promise<T>;
  }

  // Custom InputTypes

  /**
   * Space findUnique
   */
  export type SpaceFindUniqueArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * Throw an Error if a Space can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Space to fetch.
     * 
    **/
    where: SpaceWhereUniqueInput
  }


  /**
   * Space findFirst
   */
  export type SpaceFindFirstArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * Throw an Error if a Space can't be found
     * 
    **/
    rejectOnNotFound?: RejectOnNotFound
    /**
     * Filter, which Space to fetch.
     * 
    **/
    where?: SpaceWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Spaces to fetch.
     * 
    **/
    orderBy?: Enumerable<SpaceOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for searching for Spaces.
     * 
    **/
    cursor?: SpaceWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Spaces from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Spaces.
     * 
    **/
    skip?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/distinct Distinct Docs}
     * 
     * Filter by unique combinations of Spaces.
     * 
    **/
    distinct?: Enumerable<SpaceScalarFieldEnum>
  }


  /**
   * Space findMany
   */
  export type SpaceFindManyArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * Filter, which Spaces to fetch.
     * 
    **/
    where?: SpaceWhereInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/sorting Sorting Docs}
     * 
     * Determine the order of Spaces to fetch.
     * 
    **/
    orderBy?: Enumerable<SpaceOrderByWithRelationInput>
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination#cursor-based-pagination Cursor Docs}
     * 
     * Sets the position for listing Spaces.
     * 
    **/
    cursor?: SpaceWhereUniqueInput
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Take `±n` Spaces from the position of the cursor.
     * 
    **/
    take?: number
    /**
     * {@link https://www.prisma.io/docs/concepts/components/prisma-client/pagination Pagination Docs}
     * 
     * Skip the first `n` Spaces.
     * 
    **/
    skip?: number
    distinct?: Enumerable<SpaceScalarFieldEnum>
  }


  /**
   * Space create
   */
  export type SpaceCreateArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * The data needed to create a Space.
     * 
    **/
    data: XOR<SpaceCreateInput, SpaceUncheckedCreateInput>
  }


  /**
   * Space update
   */
  export type SpaceUpdateArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * The data needed to update a Space.
     * 
    **/
    data: XOR<SpaceUpdateInput, SpaceUncheckedUpdateInput>
    /**
     * Choose, which Space to update.
     * 
    **/
    where: SpaceWhereUniqueInput
  }


  /**
   * Space updateMany
   */
  export type SpaceUpdateManyArgs = {
    /**
     * The data used to update Spaces.
     * 
    **/
    data: XOR<SpaceUpdateManyMutationInput, SpaceUncheckedUpdateManyInput>
    /**
     * Filter which Spaces to update
     * 
    **/
    where?: SpaceWhereInput
  }


  /**
   * Space upsert
   */
  export type SpaceUpsertArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * The filter to search for the Space to update in case it exists.
     * 
    **/
    where: SpaceWhereUniqueInput
    /**
     * In case the Space found by the `where` argument doesn't exist, create a new Space with this data.
     * 
    **/
    create: XOR<SpaceCreateInput, SpaceUncheckedCreateInput>
    /**
     * In case the Space was found with the provided `where` argument, update it with this data.
     * 
    **/
    update: XOR<SpaceUpdateInput, SpaceUncheckedUpdateInput>
  }


  /**
   * Space delete
   */
  export type SpaceDeleteArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
    /**
     * Filter which Space to delete.
     * 
    **/
    where: SpaceWhereUniqueInput
  }


  /**
   * Space deleteMany
   */
  export type SpaceDeleteManyArgs = {
    /**
     * Filter which Spaces to delete
     * 
    **/
    where?: SpaceWhereInput
  }


  /**
   * Space without action
   */
  export type SpaceArgs = {
    /**
     * Select specific fields to fetch from the Space
     * 
    **/
    select?: SpaceSelect | null
    /**
     * Choose, which related nodes to fetch as well.
     * 
    **/
    include?: SpaceInclude | null
  }



  /**
   * Enums
   */

  // Based on
  // https://github.com/microsoft/TypeScript/issues/3192#issuecomment-261720275

  export const MigrationScalarFieldEnum: {
    id: 'id',
    name: 'name',
    checksum: 'checksum',
    steps_applied: 'steps_applied',
    applied_at: 'applied_at'
  };

  export type MigrationScalarFieldEnum = (typeof MigrationScalarFieldEnum)[keyof typeof MigrationScalarFieldEnum]


  export const LibraryScalarFieldEnum: {
    id: 'id',
    uuid: 'uuid',
    name: 'name',
    remote_id: 'remote_id',
    is_primary: 'is_primary',
    encryption: 'encryption',
    date_created: 'date_created',
    timezone: 'timezone'
  };

  export type LibraryScalarFieldEnum = (typeof LibraryScalarFieldEnum)[keyof typeof LibraryScalarFieldEnum]


  export const LibraryStatisticsScalarFieldEnum: {
    id: 'id',
    date_captured: 'date_captured',
    library_id: 'library_id',
    total_file_count: 'total_file_count',
    total_bytes_used: 'total_bytes_used',
    total_byte_capacity: 'total_byte_capacity',
    total_unique_bytes: 'total_unique_bytes'
  };

  export type LibraryStatisticsScalarFieldEnum = (typeof LibraryStatisticsScalarFieldEnum)[keyof typeof LibraryStatisticsScalarFieldEnum]


  export const ClientScalarFieldEnum: {
    id: 'id',
    uuid: 'uuid',
    name: 'name',
    platform: 'platform',
    version: 'version',
    online: 'online',
    last_seen: 'last_seen',
    timezone: 'timezone',
    date_created: 'date_created'
  };

  export type ClientScalarFieldEnum = (typeof ClientScalarFieldEnum)[keyof typeof ClientScalarFieldEnum]


  export const LocationScalarFieldEnum: {
    id: 'id',
    name: 'name',
    path: 'path',
    total_capacity: 'total_capacity',
    available_capacity: 'available_capacity',
    is_removable: 'is_removable',
    is_ejectable: 'is_ejectable',
    is_root_filesystem: 'is_root_filesystem',
    is_online: 'is_online',
    date_created: 'date_created'
  };

  export type LocationScalarFieldEnum = (typeof LocationScalarFieldEnum)[keyof typeof LocationScalarFieldEnum]


  export const FileScalarFieldEnum: {
    id: 'id',
    is_dir: 'is_dir',
    location_id: 'location_id',
    stem: 'stem',
    name: 'name',
    extension: 'extension',
    quick_checksum: 'quick_checksum',
    full_checksum: 'full_checksum',
    size_in_bytes: 'size_in_bytes',
    encryption: 'encryption',
    date_created: 'date_created',
    date_modified: 'date_modified',
    date_indexed: 'date_indexed',
    ipfs_id: 'ipfs_id',
    parent_id: 'parent_id'
  };

  export type FileScalarFieldEnum = (typeof FileScalarFieldEnum)[keyof typeof FileScalarFieldEnum]


  export const TagScalarFieldEnum: {
    id: 'id',
    name: 'name',
    encryption: 'encryption',
    total_files: 'total_files',
    redundancy_goal: 'redundancy_goal',
    date_created: 'date_created',
    date_modified: 'date_modified'
  };

  export type TagScalarFieldEnum = (typeof TagScalarFieldEnum)[keyof typeof TagScalarFieldEnum]


  export const TagOnFileScalarFieldEnum: {
    date_created: 'date_created',
    tag_id: 'tag_id',
    file_id: 'file_id'
  };

  export type TagOnFileScalarFieldEnum = (typeof TagOnFileScalarFieldEnum)[keyof typeof TagOnFileScalarFieldEnum]


  export const JobScalarFieldEnum: {
    id: 'id',
    client_id: 'client_id',
    action: 'action',
    status: 'status',
    percentage_complete: 'percentage_complete',
    task_count: 'task_count',
    completed_task_count: 'completed_task_count',
    date_created: 'date_created',
    date_modified: 'date_modified'
  };

  export type JobScalarFieldEnum = (typeof JobScalarFieldEnum)[keyof typeof JobScalarFieldEnum]


  export const SpaceScalarFieldEnum: {
    id: 'id',
    name: 'name',
    encryption: 'encryption',
    date_created: 'date_created',
    date_modified: 'date_modified',
    libraryId: 'libraryId'
  };

  export type SpaceScalarFieldEnum = (typeof SpaceScalarFieldEnum)[keyof typeof SpaceScalarFieldEnum]


  export const SortOrder: {
    asc: 'asc',
    desc: 'desc'
  };

  export type SortOrder = (typeof SortOrder)[keyof typeof SortOrder]


  /**
   * Deep Input Types
   */


  export type MigrationWhereInput = {
    AND?: Enumerable<MigrationWhereInput>
    OR?: Enumerable<MigrationWhereInput>
    NOT?: Enumerable<MigrationWhereInput>
    id?: IntFilter | number
    name?: StringFilter | string
    checksum?: StringFilter | string
    steps_applied?: IntFilter | number
    applied_at?: DateTimeFilter | Date | string
  }

  export type MigrationOrderByWithRelationInput = {
    id?: SortOrder
    name?: SortOrder
    checksum?: SortOrder
    steps_applied?: SortOrder
    applied_at?: SortOrder
  }

  export type MigrationWhereUniqueInput = {
    id?: number
    checksum?: string
  }

  export type MigrationOrderByWithAggregationInput = {
    id?: SortOrder
    name?: SortOrder
    checksum?: SortOrder
    steps_applied?: SortOrder
    applied_at?: SortOrder
    _count?: MigrationCountOrderByAggregateInput
    _avg?: MigrationAvgOrderByAggregateInput
    _max?: MigrationMaxOrderByAggregateInput
    _min?: MigrationMinOrderByAggregateInput
    _sum?: MigrationSumOrderByAggregateInput
  }

  export type MigrationScalarWhereWithAggregatesInput = {
    AND?: Enumerable<MigrationScalarWhereWithAggregatesInput>
    OR?: Enumerable<MigrationScalarWhereWithAggregatesInput>
    NOT?: Enumerable<MigrationScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    name?: StringWithAggregatesFilter | string
    checksum?: StringWithAggregatesFilter | string
    steps_applied?: IntWithAggregatesFilter | number
    applied_at?: DateTimeWithAggregatesFilter | Date | string
  }

  export type LibraryWhereInput = {
    AND?: Enumerable<LibraryWhereInput>
    OR?: Enumerable<LibraryWhereInput>
    NOT?: Enumerable<LibraryWhereInput>
    id?: IntFilter | number
    uuid?: StringFilter | string
    name?: StringFilter | string
    remote_id?: StringNullableFilter | string | null
    is_primary?: BoolFilter | boolean
    encryption?: IntFilter | number
    date_created?: DateTimeFilter | Date | string
    timezone?: StringNullableFilter | string | null
    spaces?: SpaceListRelationFilter
  }

  export type LibraryOrderByWithRelationInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    remote_id?: SortOrder
    is_primary?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    timezone?: SortOrder
    spaces?: SpaceOrderByRelationAggregateInput
  }

  export type LibraryWhereUniqueInput = {
    id?: number
    uuid?: string
  }

  export type LibraryOrderByWithAggregationInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    remote_id?: SortOrder
    is_primary?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    timezone?: SortOrder
    _count?: LibraryCountOrderByAggregateInput
    _avg?: LibraryAvgOrderByAggregateInput
    _max?: LibraryMaxOrderByAggregateInput
    _min?: LibraryMinOrderByAggregateInput
    _sum?: LibrarySumOrderByAggregateInput
  }

  export type LibraryScalarWhereWithAggregatesInput = {
    AND?: Enumerable<LibraryScalarWhereWithAggregatesInput>
    OR?: Enumerable<LibraryScalarWhereWithAggregatesInput>
    NOT?: Enumerable<LibraryScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    uuid?: StringWithAggregatesFilter | string
    name?: StringWithAggregatesFilter | string
    remote_id?: StringNullableWithAggregatesFilter | string | null
    is_primary?: BoolWithAggregatesFilter | boolean
    encryption?: IntWithAggregatesFilter | number
    date_created?: DateTimeWithAggregatesFilter | Date | string
    timezone?: StringNullableWithAggregatesFilter | string | null
  }

  export type LibraryStatisticsWhereInput = {
    AND?: Enumerable<LibraryStatisticsWhereInput>
    OR?: Enumerable<LibraryStatisticsWhereInput>
    NOT?: Enumerable<LibraryStatisticsWhereInput>
    id?: IntFilter | number
    date_captured?: DateTimeFilter | Date | string
    library_id?: IntFilter | number
    total_file_count?: IntFilter | number
    total_bytes_used?: StringFilter | string
    total_byte_capacity?: StringFilter | string
    total_unique_bytes?: StringFilter | string
  }

  export type LibraryStatisticsOrderByWithRelationInput = {
    id?: SortOrder
    date_captured?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
    total_bytes_used?: SortOrder
    total_byte_capacity?: SortOrder
    total_unique_bytes?: SortOrder
  }

  export type LibraryStatisticsWhereUniqueInput = {
    id?: number
    library_id?: number
  }

  export type LibraryStatisticsOrderByWithAggregationInput = {
    id?: SortOrder
    date_captured?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
    total_bytes_used?: SortOrder
    total_byte_capacity?: SortOrder
    total_unique_bytes?: SortOrder
    _count?: LibraryStatisticsCountOrderByAggregateInput
    _avg?: LibraryStatisticsAvgOrderByAggregateInput
    _max?: LibraryStatisticsMaxOrderByAggregateInput
    _min?: LibraryStatisticsMinOrderByAggregateInput
    _sum?: LibraryStatisticsSumOrderByAggregateInput
  }

  export type LibraryStatisticsScalarWhereWithAggregatesInput = {
    AND?: Enumerable<LibraryStatisticsScalarWhereWithAggregatesInput>
    OR?: Enumerable<LibraryStatisticsScalarWhereWithAggregatesInput>
    NOT?: Enumerable<LibraryStatisticsScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    date_captured?: DateTimeWithAggregatesFilter | Date | string
    library_id?: IntWithAggregatesFilter | number
    total_file_count?: IntWithAggregatesFilter | number
    total_bytes_used?: StringWithAggregatesFilter | string
    total_byte_capacity?: StringWithAggregatesFilter | string
    total_unique_bytes?: StringWithAggregatesFilter | string
  }

  export type ClientWhereInput = {
    AND?: Enumerable<ClientWhereInput>
    OR?: Enumerable<ClientWhereInput>
    NOT?: Enumerable<ClientWhereInput>
    id?: IntFilter | number
    uuid?: StringFilter | string
    name?: StringFilter | string
    platform?: IntFilter | number
    version?: StringNullableFilter | string | null
    online?: BoolNullableFilter | boolean | null
    last_seen?: DateTimeFilter | Date | string
    timezone?: StringNullableFilter | string | null
    date_created?: DateTimeFilter | Date | string
    jobs?: JobListRelationFilter
  }

  export type ClientOrderByWithRelationInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    platform?: SortOrder
    version?: SortOrder
    online?: SortOrder
    last_seen?: SortOrder
    timezone?: SortOrder
    date_created?: SortOrder
    jobs?: JobOrderByRelationAggregateInput
  }

  export type ClientWhereUniqueInput = {
    id?: number
    uuid?: string
  }

  export type ClientOrderByWithAggregationInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    platform?: SortOrder
    version?: SortOrder
    online?: SortOrder
    last_seen?: SortOrder
    timezone?: SortOrder
    date_created?: SortOrder
    _count?: ClientCountOrderByAggregateInput
    _avg?: ClientAvgOrderByAggregateInput
    _max?: ClientMaxOrderByAggregateInput
    _min?: ClientMinOrderByAggregateInput
    _sum?: ClientSumOrderByAggregateInput
  }

  export type ClientScalarWhereWithAggregatesInput = {
    AND?: Enumerable<ClientScalarWhereWithAggregatesInput>
    OR?: Enumerable<ClientScalarWhereWithAggregatesInput>
    NOT?: Enumerable<ClientScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    uuid?: StringWithAggregatesFilter | string
    name?: StringWithAggregatesFilter | string
    platform?: IntWithAggregatesFilter | number
    version?: StringNullableWithAggregatesFilter | string | null
    online?: BoolNullableWithAggregatesFilter | boolean | null
    last_seen?: DateTimeWithAggregatesFilter | Date | string
    timezone?: StringNullableWithAggregatesFilter | string | null
    date_created?: DateTimeWithAggregatesFilter | Date | string
  }

  export type LocationWhereInput = {
    AND?: Enumerable<LocationWhereInput>
    OR?: Enumerable<LocationWhereInput>
    NOT?: Enumerable<LocationWhereInput>
    id?: IntFilter | number
    name?: StringNullableFilter | string | null
    path?: StringNullableFilter | string | null
    total_capacity?: IntNullableFilter | number | null
    available_capacity?: IntNullableFilter | number | null
    is_removable?: BoolFilter | boolean
    is_ejectable?: BoolFilter | boolean
    is_root_filesystem?: BoolFilter | boolean
    is_online?: BoolFilter | boolean
    date_created?: DateTimeFilter | Date | string
    files?: FileListRelationFilter
  }

  export type LocationOrderByWithRelationInput = {
    id?: SortOrder
    name?: SortOrder
    path?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
    is_removable?: SortOrder
    is_ejectable?: SortOrder
    is_root_filesystem?: SortOrder
    is_online?: SortOrder
    date_created?: SortOrder
    files?: FileOrderByRelationAggregateInput
  }

  export type LocationWhereUniqueInput = {
    id?: number
  }

  export type LocationOrderByWithAggregationInput = {
    id?: SortOrder
    name?: SortOrder
    path?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
    is_removable?: SortOrder
    is_ejectable?: SortOrder
    is_root_filesystem?: SortOrder
    is_online?: SortOrder
    date_created?: SortOrder
    _count?: LocationCountOrderByAggregateInput
    _avg?: LocationAvgOrderByAggregateInput
    _max?: LocationMaxOrderByAggregateInput
    _min?: LocationMinOrderByAggregateInput
    _sum?: LocationSumOrderByAggregateInput
  }

  export type LocationScalarWhereWithAggregatesInput = {
    AND?: Enumerable<LocationScalarWhereWithAggregatesInput>
    OR?: Enumerable<LocationScalarWhereWithAggregatesInput>
    NOT?: Enumerable<LocationScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    name?: StringNullableWithAggregatesFilter | string | null
    path?: StringNullableWithAggregatesFilter | string | null
    total_capacity?: IntNullableWithAggregatesFilter | number | null
    available_capacity?: IntNullableWithAggregatesFilter | number | null
    is_removable?: BoolWithAggregatesFilter | boolean
    is_ejectable?: BoolWithAggregatesFilter | boolean
    is_root_filesystem?: BoolWithAggregatesFilter | boolean
    is_online?: BoolWithAggregatesFilter | boolean
    date_created?: DateTimeWithAggregatesFilter | Date | string
  }

  export type FileWhereInput = {
    AND?: Enumerable<FileWhereInput>
    OR?: Enumerable<FileWhereInput>
    NOT?: Enumerable<FileWhereInput>
    id?: IntFilter | number
    is_dir?: BoolFilter | boolean
    location_id?: IntFilter | number
    stem?: StringFilter | string
    name?: StringFilter | string
    extension?: StringNullableFilter | string | null
    quick_checksum?: StringNullableFilter | string | null
    full_checksum?: StringNullableFilter | string | null
    size_in_bytes?: StringFilter | string
    encryption?: IntFilter | number
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
    date_indexed?: DateTimeFilter | Date | string
    ipfs_id?: StringNullableFilter | string | null
    location?: XOR<LocationRelationFilter, LocationWhereInput> | null
    parent?: XOR<FileRelationFilter, FileWhereInput> | null
    parent_id?: IntNullableFilter | number | null
    children?: FileListRelationFilter
    file_tags?: TagOnFileListRelationFilter
  }

  export type FileOrderByWithRelationInput = {
    id?: SortOrder
    is_dir?: SortOrder
    location_id?: SortOrder
    stem?: SortOrder
    name?: SortOrder
    extension?: SortOrder
    quick_checksum?: SortOrder
    full_checksum?: SortOrder
    size_in_bytes?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    date_indexed?: SortOrder
    ipfs_id?: SortOrder
    location?: LocationOrderByWithRelationInput
    parent?: FileOrderByWithRelationInput
    parent_id?: SortOrder
    children?: FileOrderByRelationAggregateInput
    file_tags?: TagOnFileOrderByRelationAggregateInput
  }

  export type FileWhereUniqueInput = {
    id?: number
    location_id_stem_name_extension?: FileLocation_idStemNameExtensionCompoundUniqueInput
  }

  export type FileOrderByWithAggregationInput = {
    id?: SortOrder
    is_dir?: SortOrder
    location_id?: SortOrder
    stem?: SortOrder
    name?: SortOrder
    extension?: SortOrder
    quick_checksum?: SortOrder
    full_checksum?: SortOrder
    size_in_bytes?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    date_indexed?: SortOrder
    ipfs_id?: SortOrder
    parent_id?: SortOrder
    _count?: FileCountOrderByAggregateInput
    _avg?: FileAvgOrderByAggregateInput
    _max?: FileMaxOrderByAggregateInput
    _min?: FileMinOrderByAggregateInput
    _sum?: FileSumOrderByAggregateInput
  }

  export type FileScalarWhereWithAggregatesInput = {
    AND?: Enumerable<FileScalarWhereWithAggregatesInput>
    OR?: Enumerable<FileScalarWhereWithAggregatesInput>
    NOT?: Enumerable<FileScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    is_dir?: BoolWithAggregatesFilter | boolean
    location_id?: IntWithAggregatesFilter | number
    stem?: StringWithAggregatesFilter | string
    name?: StringWithAggregatesFilter | string
    extension?: StringNullableWithAggregatesFilter | string | null
    quick_checksum?: StringNullableWithAggregatesFilter | string | null
    full_checksum?: StringNullableWithAggregatesFilter | string | null
    size_in_bytes?: StringWithAggregatesFilter | string
    encryption?: IntWithAggregatesFilter | number
    date_created?: DateTimeWithAggregatesFilter | Date | string
    date_modified?: DateTimeWithAggregatesFilter | Date | string
    date_indexed?: DateTimeWithAggregatesFilter | Date | string
    ipfs_id?: StringNullableWithAggregatesFilter | string | null
    parent_id?: IntNullableWithAggregatesFilter | number | null
  }

  export type TagWhereInput = {
    AND?: Enumerable<TagWhereInput>
    OR?: Enumerable<TagWhereInput>
    NOT?: Enumerable<TagWhereInput>
    id?: IntFilter | number
    name?: StringNullableFilter | string | null
    encryption?: IntNullableFilter | number | null
    total_files?: IntNullableFilter | number | null
    redundancy_goal?: IntNullableFilter | number | null
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
    tag_files?: TagOnFileListRelationFilter
  }

  export type TagOrderByWithRelationInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    tag_files?: TagOnFileOrderByRelationAggregateInput
  }

  export type TagWhereUniqueInput = {
    id?: number
  }

  export type TagOrderByWithAggregationInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    _count?: TagCountOrderByAggregateInput
    _avg?: TagAvgOrderByAggregateInput
    _max?: TagMaxOrderByAggregateInput
    _min?: TagMinOrderByAggregateInput
    _sum?: TagSumOrderByAggregateInput
  }

  export type TagScalarWhereWithAggregatesInput = {
    AND?: Enumerable<TagScalarWhereWithAggregatesInput>
    OR?: Enumerable<TagScalarWhereWithAggregatesInput>
    NOT?: Enumerable<TagScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    name?: StringNullableWithAggregatesFilter | string | null
    encryption?: IntNullableWithAggregatesFilter | number | null
    total_files?: IntNullableWithAggregatesFilter | number | null
    redundancy_goal?: IntNullableWithAggregatesFilter | number | null
    date_created?: DateTimeWithAggregatesFilter | Date | string
    date_modified?: DateTimeWithAggregatesFilter | Date | string
  }

  export type TagOnFileWhereInput = {
    AND?: Enumerable<TagOnFileWhereInput>
    OR?: Enumerable<TagOnFileWhereInput>
    NOT?: Enumerable<TagOnFileWhereInput>
    date_created?: DateTimeFilter | Date | string
    tag_id?: IntFilter | number
    tag?: XOR<TagRelationFilter, TagWhereInput>
    file_id?: IntFilter | number
    file?: XOR<FileRelationFilter, FileWhereInput>
  }

  export type TagOnFileOrderByWithRelationInput = {
    date_created?: SortOrder
    tag_id?: SortOrder
    tag?: TagOrderByWithRelationInput
    file_id?: SortOrder
    file?: FileOrderByWithRelationInput
  }

  export type TagOnFileWhereUniqueInput = {
    tag_id_file_id?: TagOnFileTag_idFile_idCompoundUniqueInput
  }

  export type TagOnFileOrderByWithAggregationInput = {
    date_created?: SortOrder
    tag_id?: SortOrder
    file_id?: SortOrder
    _count?: TagOnFileCountOrderByAggregateInput
    _avg?: TagOnFileAvgOrderByAggregateInput
    _max?: TagOnFileMaxOrderByAggregateInput
    _min?: TagOnFileMinOrderByAggregateInput
    _sum?: TagOnFileSumOrderByAggregateInput
  }

  export type TagOnFileScalarWhereWithAggregatesInput = {
    AND?: Enumerable<TagOnFileScalarWhereWithAggregatesInput>
    OR?: Enumerable<TagOnFileScalarWhereWithAggregatesInput>
    NOT?: Enumerable<TagOnFileScalarWhereWithAggregatesInput>
    date_created?: DateTimeWithAggregatesFilter | Date | string
    tag_id?: IntWithAggregatesFilter | number
    file_id?: IntWithAggregatesFilter | number
  }

  export type JobWhereInput = {
    AND?: Enumerable<JobWhereInput>
    OR?: Enumerable<JobWhereInput>
    NOT?: Enumerable<JobWhereInput>
    id?: IntFilter | number
    client_id?: IntFilter | number
    action?: IntFilter | number
    status?: IntFilter | number
    percentage_complete?: IntFilter | number
    task_count?: IntFilter | number
    completed_task_count?: IntFilter | number
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
    clients?: XOR<ClientRelationFilter, ClientWhereInput>
  }

  export type JobOrderByWithRelationInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    clients?: ClientOrderByWithRelationInput
  }

  export type JobWhereUniqueInput = {
    id?: number
  }

  export type JobOrderByWithAggregationInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    _count?: JobCountOrderByAggregateInput
    _avg?: JobAvgOrderByAggregateInput
    _max?: JobMaxOrderByAggregateInput
    _min?: JobMinOrderByAggregateInput
    _sum?: JobSumOrderByAggregateInput
  }

  export type JobScalarWhereWithAggregatesInput = {
    AND?: Enumerable<JobScalarWhereWithAggregatesInput>
    OR?: Enumerable<JobScalarWhereWithAggregatesInput>
    NOT?: Enumerable<JobScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    client_id?: IntWithAggregatesFilter | number
    action?: IntWithAggregatesFilter | number
    status?: IntWithAggregatesFilter | number
    percentage_complete?: IntWithAggregatesFilter | number
    task_count?: IntWithAggregatesFilter | number
    completed_task_count?: IntWithAggregatesFilter | number
    date_created?: DateTimeWithAggregatesFilter | Date | string
    date_modified?: DateTimeWithAggregatesFilter | Date | string
  }

  export type SpaceWhereInput = {
    AND?: Enumerable<SpaceWhereInput>
    OR?: Enumerable<SpaceWhereInput>
    NOT?: Enumerable<SpaceWhereInput>
    id?: IntFilter | number
    name?: StringFilter | string
    encryption?: IntNullableFilter | number | null
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
    Library?: XOR<LibraryRelationFilter, LibraryWhereInput> | null
    libraryId?: IntNullableFilter | number | null
  }

  export type SpaceOrderByWithRelationInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    Library?: LibraryOrderByWithRelationInput
    libraryId?: SortOrder
  }

  export type SpaceWhereUniqueInput = {
    id?: number
  }

  export type SpaceOrderByWithAggregationInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    libraryId?: SortOrder
    _count?: SpaceCountOrderByAggregateInput
    _avg?: SpaceAvgOrderByAggregateInput
    _max?: SpaceMaxOrderByAggregateInput
    _min?: SpaceMinOrderByAggregateInput
    _sum?: SpaceSumOrderByAggregateInput
  }

  export type SpaceScalarWhereWithAggregatesInput = {
    AND?: Enumerable<SpaceScalarWhereWithAggregatesInput>
    OR?: Enumerable<SpaceScalarWhereWithAggregatesInput>
    NOT?: Enumerable<SpaceScalarWhereWithAggregatesInput>
    id?: IntWithAggregatesFilter | number
    name?: StringWithAggregatesFilter | string
    encryption?: IntNullableWithAggregatesFilter | number | null
    date_created?: DateTimeWithAggregatesFilter | Date | string
    date_modified?: DateTimeWithAggregatesFilter | Date | string
    libraryId?: IntNullableWithAggregatesFilter | number | null
  }

  export type MigrationCreateInput = {
    name: string
    checksum: string
    steps_applied?: number
    applied_at?: Date | string
  }

  export type MigrationUncheckedCreateInput = {
    id?: number
    name: string
    checksum: string
    steps_applied?: number
    applied_at?: Date | string
  }

  export type MigrationUpdateInput = {
    name?: StringFieldUpdateOperationsInput | string
    checksum?: StringFieldUpdateOperationsInput | string
    steps_applied?: IntFieldUpdateOperationsInput | number
    applied_at?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type MigrationUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: StringFieldUpdateOperationsInput | string
    checksum?: StringFieldUpdateOperationsInput | string
    steps_applied?: IntFieldUpdateOperationsInput | number
    applied_at?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type MigrationUpdateManyMutationInput = {
    name?: StringFieldUpdateOperationsInput | string
    checksum?: StringFieldUpdateOperationsInput | string
    steps_applied?: IntFieldUpdateOperationsInput | number
    applied_at?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type MigrationUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: StringFieldUpdateOperationsInput | string
    checksum?: StringFieldUpdateOperationsInput | string
    steps_applied?: IntFieldUpdateOperationsInput | number
    applied_at?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type LibraryCreateInput = {
    uuid: string
    name: string
    remote_id?: string | null
    is_primary?: boolean
    encryption?: number
    date_created?: Date | string
    timezone?: string | null
    spaces?: SpaceCreateNestedManyWithoutLibraryInput
  }

  export type LibraryUncheckedCreateInput = {
    id?: number
    uuid: string
    name: string
    remote_id?: string | null
    is_primary?: boolean
    encryption?: number
    date_created?: Date | string
    timezone?: string | null
    spaces?: SpaceUncheckedCreateNestedManyWithoutLibraryInput
  }

  export type LibraryUpdateInput = {
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    remote_id?: NullableStringFieldUpdateOperationsInput | string | null
    is_primary?: BoolFieldUpdateOperationsInput | boolean
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    spaces?: SpaceUpdateManyWithoutLibraryInput
  }

  export type LibraryUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    remote_id?: NullableStringFieldUpdateOperationsInput | string | null
    is_primary?: BoolFieldUpdateOperationsInput | boolean
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    spaces?: SpaceUncheckedUpdateManyWithoutLibraryInput
  }

  export type LibraryUpdateManyMutationInput = {
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    remote_id?: NullableStringFieldUpdateOperationsInput | string | null
    is_primary?: BoolFieldUpdateOperationsInput | boolean
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
  }

  export type LibraryUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    remote_id?: NullableStringFieldUpdateOperationsInput | string | null
    is_primary?: BoolFieldUpdateOperationsInput | boolean
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
  }

  export type LibraryStatisticsCreateInput = {
    date_captured?: Date | string
    library_id: number
    total_file_count?: number
    total_bytes_used?: string
    total_byte_capacity?: string
    total_unique_bytes?: string
  }

  export type LibraryStatisticsUncheckedCreateInput = {
    id?: number
    date_captured?: Date | string
    library_id: number
    total_file_count?: number
    total_bytes_used?: string
    total_byte_capacity?: string
    total_unique_bytes?: string
  }

  export type LibraryStatisticsUpdateInput = {
    date_captured?: DateTimeFieldUpdateOperationsInput | Date | string
    library_id?: IntFieldUpdateOperationsInput | number
    total_file_count?: IntFieldUpdateOperationsInput | number
    total_bytes_used?: StringFieldUpdateOperationsInput | string
    total_byte_capacity?: StringFieldUpdateOperationsInput | string
    total_unique_bytes?: StringFieldUpdateOperationsInput | string
  }

  export type LibraryStatisticsUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    date_captured?: DateTimeFieldUpdateOperationsInput | Date | string
    library_id?: IntFieldUpdateOperationsInput | number
    total_file_count?: IntFieldUpdateOperationsInput | number
    total_bytes_used?: StringFieldUpdateOperationsInput | string
    total_byte_capacity?: StringFieldUpdateOperationsInput | string
    total_unique_bytes?: StringFieldUpdateOperationsInput | string
  }

  export type LibraryStatisticsUpdateManyMutationInput = {
    date_captured?: DateTimeFieldUpdateOperationsInput | Date | string
    library_id?: IntFieldUpdateOperationsInput | number
    total_file_count?: IntFieldUpdateOperationsInput | number
    total_bytes_used?: StringFieldUpdateOperationsInput | string
    total_byte_capacity?: StringFieldUpdateOperationsInput | string
    total_unique_bytes?: StringFieldUpdateOperationsInput | string
  }

  export type LibraryStatisticsUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    date_captured?: DateTimeFieldUpdateOperationsInput | Date | string
    library_id?: IntFieldUpdateOperationsInput | number
    total_file_count?: IntFieldUpdateOperationsInput | number
    total_bytes_used?: StringFieldUpdateOperationsInput | string
    total_byte_capacity?: StringFieldUpdateOperationsInput | string
    total_unique_bytes?: StringFieldUpdateOperationsInput | string
  }

  export type ClientCreateInput = {
    uuid: string
    name: string
    platform?: number
    version?: string | null
    online?: boolean | null
    last_seen?: Date | string
    timezone?: string | null
    date_created?: Date | string
    jobs?: JobCreateNestedManyWithoutClientsInput
  }

  export type ClientUncheckedCreateInput = {
    id?: number
    uuid: string
    name: string
    platform?: number
    version?: string | null
    online?: boolean | null
    last_seen?: Date | string
    timezone?: string | null
    date_created?: Date | string
    jobs?: JobUncheckedCreateNestedManyWithoutClientsInput
  }

  export type ClientUpdateInput = {
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    platform?: IntFieldUpdateOperationsInput | number
    version?: NullableStringFieldUpdateOperationsInput | string | null
    online?: NullableBoolFieldUpdateOperationsInput | boolean | null
    last_seen?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    jobs?: JobUpdateManyWithoutClientsInput
  }

  export type ClientUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    platform?: IntFieldUpdateOperationsInput | number
    version?: NullableStringFieldUpdateOperationsInput | string | null
    online?: NullableBoolFieldUpdateOperationsInput | boolean | null
    last_seen?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    jobs?: JobUncheckedUpdateManyWithoutClientsInput
  }

  export type ClientUpdateManyMutationInput = {
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    platform?: IntFieldUpdateOperationsInput | number
    version?: NullableStringFieldUpdateOperationsInput | string | null
    online?: NullableBoolFieldUpdateOperationsInput | boolean | null
    last_seen?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type ClientUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    platform?: IntFieldUpdateOperationsInput | number
    version?: NullableStringFieldUpdateOperationsInput | string | null
    online?: NullableBoolFieldUpdateOperationsInput | boolean | null
    last_seen?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type LocationCreateInput = {
    name?: string | null
    path?: string | null
    total_capacity?: number | null
    available_capacity?: number | null
    is_removable?: boolean
    is_ejectable?: boolean
    is_root_filesystem?: boolean
    is_online?: boolean
    date_created?: Date | string
    files?: FileCreateNestedManyWithoutLocationInput
  }

  export type LocationUncheckedCreateInput = {
    id?: number
    name?: string | null
    path?: string | null
    total_capacity?: number | null
    available_capacity?: number | null
    is_removable?: boolean
    is_ejectable?: boolean
    is_root_filesystem?: boolean
    is_online?: boolean
    date_created?: Date | string
    files?: FileUncheckedCreateNestedManyWithoutLocationInput
  }

  export type LocationUpdateInput = {
    name?: NullableStringFieldUpdateOperationsInput | string | null
    path?: NullableStringFieldUpdateOperationsInput | string | null
    total_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    available_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    is_removable?: BoolFieldUpdateOperationsInput | boolean
    is_ejectable?: BoolFieldUpdateOperationsInput | boolean
    is_root_filesystem?: BoolFieldUpdateOperationsInput | boolean
    is_online?: BoolFieldUpdateOperationsInput | boolean
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    files?: FileUpdateManyWithoutLocationInput
  }

  export type LocationUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: NullableStringFieldUpdateOperationsInput | string | null
    path?: NullableStringFieldUpdateOperationsInput | string | null
    total_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    available_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    is_removable?: BoolFieldUpdateOperationsInput | boolean
    is_ejectable?: BoolFieldUpdateOperationsInput | boolean
    is_root_filesystem?: BoolFieldUpdateOperationsInput | boolean
    is_online?: BoolFieldUpdateOperationsInput | boolean
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    files?: FileUncheckedUpdateManyWithoutLocationInput
  }

  export type LocationUpdateManyMutationInput = {
    name?: NullableStringFieldUpdateOperationsInput | string | null
    path?: NullableStringFieldUpdateOperationsInput | string | null
    total_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    available_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    is_removable?: BoolFieldUpdateOperationsInput | boolean
    is_ejectable?: BoolFieldUpdateOperationsInput | boolean
    is_root_filesystem?: BoolFieldUpdateOperationsInput | boolean
    is_online?: BoolFieldUpdateOperationsInput | boolean
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type LocationUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: NullableStringFieldUpdateOperationsInput | string | null
    path?: NullableStringFieldUpdateOperationsInput | string | null
    total_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    available_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    is_removable?: BoolFieldUpdateOperationsInput | boolean
    is_ejectable?: BoolFieldUpdateOperationsInput | boolean
    is_root_filesystem?: BoolFieldUpdateOperationsInput | boolean
    is_online?: BoolFieldUpdateOperationsInput | boolean
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type FileCreateInput = {
    is_dir?: boolean
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    location?: LocationCreateNestedOneWithoutFilesInput
    parent?: FileCreateNestedOneWithoutChildrenInput
    children?: FileCreateNestedManyWithoutParentInput
    file_tags?: TagOnFileCreateNestedManyWithoutFileInput
  }

  export type FileUncheckedCreateInput = {
    id?: number
    is_dir?: boolean
    location_id: number
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    parent_id?: number | null
    children?: FileUncheckedCreateNestedManyWithoutParentInput
    file_tags?: TagOnFileUncheckedCreateNestedManyWithoutFileInput
  }

  export type FileUpdateInput = {
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    location?: LocationUpdateOneWithoutFilesInput
    parent?: FileUpdateOneWithoutChildrenInput
    children?: FileUpdateManyWithoutParentInput
    file_tags?: TagOnFileUpdateManyWithoutFileInput
  }

  export type FileUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    location_id?: IntFieldUpdateOperationsInput | number
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent_id?: NullableIntFieldUpdateOperationsInput | number | null
    children?: FileUncheckedUpdateManyWithoutParentInput
    file_tags?: TagOnFileUncheckedUpdateManyWithoutFileInput
  }

  export type FileUpdateManyMutationInput = {
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
  }

  export type FileUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    location_id?: IntFieldUpdateOperationsInput | number
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent_id?: NullableIntFieldUpdateOperationsInput | number | null
  }

  export type TagCreateInput = {
    name?: string | null
    encryption?: number | null
    total_files?: number | null
    redundancy_goal?: number | null
    date_created?: Date | string
    date_modified?: Date | string
    tag_files?: TagOnFileCreateNestedManyWithoutTagInput
  }

  export type TagUncheckedCreateInput = {
    id?: number
    name?: string | null
    encryption?: number | null
    total_files?: number | null
    redundancy_goal?: number | null
    date_created?: Date | string
    date_modified?: Date | string
    tag_files?: TagOnFileUncheckedCreateNestedManyWithoutTagInput
  }

  export type TagUpdateInput = {
    name?: NullableStringFieldUpdateOperationsInput | string | null
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    total_files?: NullableIntFieldUpdateOperationsInput | number | null
    redundancy_goal?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    tag_files?: TagOnFileUpdateManyWithoutTagInput
  }

  export type TagUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: NullableStringFieldUpdateOperationsInput | string | null
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    total_files?: NullableIntFieldUpdateOperationsInput | number | null
    redundancy_goal?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    tag_files?: TagOnFileUncheckedUpdateManyWithoutTagInput
  }

  export type TagUpdateManyMutationInput = {
    name?: NullableStringFieldUpdateOperationsInput | string | null
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    total_files?: NullableIntFieldUpdateOperationsInput | number | null
    redundancy_goal?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type TagUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: NullableStringFieldUpdateOperationsInput | string | null
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    total_files?: NullableIntFieldUpdateOperationsInput | number | null
    redundancy_goal?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type TagOnFileCreateInput = {
    date_created?: Date | string
    tag: TagCreateNestedOneWithoutTag_filesInput
    file: FileCreateNestedOneWithoutFile_tagsInput
  }

  export type TagOnFileUncheckedCreateInput = {
    date_created?: Date | string
    tag_id: number
    file_id: number
  }

  export type TagOnFileUpdateInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    tag?: TagUpdateOneRequiredWithoutTag_filesInput
    file?: FileUpdateOneRequiredWithoutFile_tagsInput
  }

  export type TagOnFileUncheckedUpdateInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    tag_id?: IntFieldUpdateOperationsInput | number
    file_id?: IntFieldUpdateOperationsInput | number
  }

  export type TagOnFileUpdateManyMutationInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type TagOnFileUncheckedUpdateManyInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    tag_id?: IntFieldUpdateOperationsInput | number
    file_id?: IntFieldUpdateOperationsInput | number
  }

  export type JobCreateInput = {
    action: number
    status?: number
    percentage_complete?: number
    task_count?: number
    completed_task_count?: number
    date_created?: Date | string
    date_modified?: Date | string
    clients: ClientCreateNestedOneWithoutJobsInput
  }

  export type JobUncheckedCreateInput = {
    id?: number
    client_id: number
    action: number
    status?: number
    percentage_complete?: number
    task_count?: number
    completed_task_count?: number
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type JobUpdateInput = {
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    clients?: ClientUpdateOneRequiredWithoutJobsInput
  }

  export type JobUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    client_id?: IntFieldUpdateOperationsInput | number
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type JobUpdateManyMutationInput = {
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type JobUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    client_id?: IntFieldUpdateOperationsInput | number
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type SpaceCreateInput = {
    name: string
    encryption?: number | null
    date_created?: Date | string
    date_modified?: Date | string
    Library?: LibraryCreateNestedOneWithoutSpacesInput
  }

  export type SpaceUncheckedCreateInput = {
    id?: number
    name: string
    encryption?: number | null
    date_created?: Date | string
    date_modified?: Date | string
    libraryId?: number | null
  }

  export type SpaceUpdateInput = {
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    Library?: LibraryUpdateOneWithoutSpacesInput
  }

  export type SpaceUncheckedUpdateInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    libraryId?: NullableIntFieldUpdateOperationsInput | number | null
  }

  export type SpaceUpdateManyMutationInput = {
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type SpaceUncheckedUpdateManyInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    libraryId?: NullableIntFieldUpdateOperationsInput | number | null
  }

  export type IntFilter = {
    equals?: number
    in?: Enumerable<number>
    notIn?: Enumerable<number>
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntFilter | number
  }

  export type StringFilter = {
    equals?: string
    in?: Enumerable<string>
    notIn?: Enumerable<string>
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringFilter | string
  }

  export type DateTimeFilter = {
    equals?: Date | string
    in?: Enumerable<Date> | Enumerable<string>
    notIn?: Enumerable<Date> | Enumerable<string>
    lt?: Date | string
    lte?: Date | string
    gt?: Date | string
    gte?: Date | string
    not?: NestedDateTimeFilter | Date | string
  }

  export type MigrationCountOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    checksum?: SortOrder
    steps_applied?: SortOrder
    applied_at?: SortOrder
  }

  export type MigrationAvgOrderByAggregateInput = {
    id?: SortOrder
    steps_applied?: SortOrder
  }

  export type MigrationMaxOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    checksum?: SortOrder
    steps_applied?: SortOrder
    applied_at?: SortOrder
  }

  export type MigrationMinOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    checksum?: SortOrder
    steps_applied?: SortOrder
    applied_at?: SortOrder
  }

  export type MigrationSumOrderByAggregateInput = {
    id?: SortOrder
    steps_applied?: SortOrder
  }

  export type IntWithAggregatesFilter = {
    equals?: number
    in?: Enumerable<number>
    notIn?: Enumerable<number>
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntWithAggregatesFilter | number
    _count?: NestedIntFilter
    _avg?: NestedFloatFilter
    _sum?: NestedIntFilter
    _min?: NestedIntFilter
    _max?: NestedIntFilter
  }

  export type StringWithAggregatesFilter = {
    equals?: string
    in?: Enumerable<string>
    notIn?: Enumerable<string>
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringWithAggregatesFilter | string
    _count?: NestedIntFilter
    _min?: NestedStringFilter
    _max?: NestedStringFilter
  }

  export type DateTimeWithAggregatesFilter = {
    equals?: Date | string
    in?: Enumerable<Date> | Enumerable<string>
    notIn?: Enumerable<Date> | Enumerable<string>
    lt?: Date | string
    lte?: Date | string
    gt?: Date | string
    gte?: Date | string
    not?: NestedDateTimeWithAggregatesFilter | Date | string
    _count?: NestedIntFilter
    _min?: NestedDateTimeFilter
    _max?: NestedDateTimeFilter
  }

  export type StringNullableFilter = {
    equals?: string | null
    in?: Enumerable<string> | null
    notIn?: Enumerable<string> | null
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringNullableFilter | string | null
  }

  export type BoolFilter = {
    equals?: boolean
    not?: NestedBoolFilter | boolean
  }

  export type SpaceListRelationFilter = {
    every?: SpaceWhereInput
    some?: SpaceWhereInput
    none?: SpaceWhereInput
  }

  export type SpaceOrderByRelationAggregateInput = {
    _count?: SortOrder
  }

  export type LibraryCountOrderByAggregateInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    remote_id?: SortOrder
    is_primary?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    timezone?: SortOrder
  }

  export type LibraryAvgOrderByAggregateInput = {
    id?: SortOrder
    encryption?: SortOrder
  }

  export type LibraryMaxOrderByAggregateInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    remote_id?: SortOrder
    is_primary?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    timezone?: SortOrder
  }

  export type LibraryMinOrderByAggregateInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    remote_id?: SortOrder
    is_primary?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    timezone?: SortOrder
  }

  export type LibrarySumOrderByAggregateInput = {
    id?: SortOrder
    encryption?: SortOrder
  }

  export type StringNullableWithAggregatesFilter = {
    equals?: string | null
    in?: Enumerable<string> | null
    notIn?: Enumerable<string> | null
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringNullableWithAggregatesFilter | string | null
    _count?: NestedIntNullableFilter
    _min?: NestedStringNullableFilter
    _max?: NestedStringNullableFilter
  }

  export type BoolWithAggregatesFilter = {
    equals?: boolean
    not?: NestedBoolWithAggregatesFilter | boolean
    _count?: NestedIntFilter
    _min?: NestedBoolFilter
    _max?: NestedBoolFilter
  }

  export type LibraryStatisticsCountOrderByAggregateInput = {
    id?: SortOrder
    date_captured?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
    total_bytes_used?: SortOrder
    total_byte_capacity?: SortOrder
    total_unique_bytes?: SortOrder
  }

  export type LibraryStatisticsAvgOrderByAggregateInput = {
    id?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
  }

  export type LibraryStatisticsMaxOrderByAggregateInput = {
    id?: SortOrder
    date_captured?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
    total_bytes_used?: SortOrder
    total_byte_capacity?: SortOrder
    total_unique_bytes?: SortOrder
  }

  export type LibraryStatisticsMinOrderByAggregateInput = {
    id?: SortOrder
    date_captured?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
    total_bytes_used?: SortOrder
    total_byte_capacity?: SortOrder
    total_unique_bytes?: SortOrder
  }

  export type LibraryStatisticsSumOrderByAggregateInput = {
    id?: SortOrder
    library_id?: SortOrder
    total_file_count?: SortOrder
  }

  export type BoolNullableFilter = {
    equals?: boolean | null
    not?: NestedBoolNullableFilter | boolean | null
  }

  export type JobListRelationFilter = {
    every?: JobWhereInput
    some?: JobWhereInput
    none?: JobWhereInput
  }

  export type JobOrderByRelationAggregateInput = {
    _count?: SortOrder
  }

  export type ClientCountOrderByAggregateInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    platform?: SortOrder
    version?: SortOrder
    online?: SortOrder
    last_seen?: SortOrder
    timezone?: SortOrder
    date_created?: SortOrder
  }

  export type ClientAvgOrderByAggregateInput = {
    id?: SortOrder
    platform?: SortOrder
  }

  export type ClientMaxOrderByAggregateInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    platform?: SortOrder
    version?: SortOrder
    online?: SortOrder
    last_seen?: SortOrder
    timezone?: SortOrder
    date_created?: SortOrder
  }

  export type ClientMinOrderByAggregateInput = {
    id?: SortOrder
    uuid?: SortOrder
    name?: SortOrder
    platform?: SortOrder
    version?: SortOrder
    online?: SortOrder
    last_seen?: SortOrder
    timezone?: SortOrder
    date_created?: SortOrder
  }

  export type ClientSumOrderByAggregateInput = {
    id?: SortOrder
    platform?: SortOrder
  }

  export type BoolNullableWithAggregatesFilter = {
    equals?: boolean | null
    not?: NestedBoolNullableWithAggregatesFilter | boolean | null
    _count?: NestedIntNullableFilter
    _min?: NestedBoolNullableFilter
    _max?: NestedBoolNullableFilter
  }

  export type IntNullableFilter = {
    equals?: number | null
    in?: Enumerable<number> | null
    notIn?: Enumerable<number> | null
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntNullableFilter | number | null
  }

  export type FileListRelationFilter = {
    every?: FileWhereInput
    some?: FileWhereInput
    none?: FileWhereInput
  }

  export type FileOrderByRelationAggregateInput = {
    _count?: SortOrder
  }

  export type LocationCountOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    path?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
    is_removable?: SortOrder
    is_ejectable?: SortOrder
    is_root_filesystem?: SortOrder
    is_online?: SortOrder
    date_created?: SortOrder
  }

  export type LocationAvgOrderByAggregateInput = {
    id?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
  }

  export type LocationMaxOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    path?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
    is_removable?: SortOrder
    is_ejectable?: SortOrder
    is_root_filesystem?: SortOrder
    is_online?: SortOrder
    date_created?: SortOrder
  }

  export type LocationMinOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    path?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
    is_removable?: SortOrder
    is_ejectable?: SortOrder
    is_root_filesystem?: SortOrder
    is_online?: SortOrder
    date_created?: SortOrder
  }

  export type LocationSumOrderByAggregateInput = {
    id?: SortOrder
    total_capacity?: SortOrder
    available_capacity?: SortOrder
  }

  export type IntNullableWithAggregatesFilter = {
    equals?: number | null
    in?: Enumerable<number> | null
    notIn?: Enumerable<number> | null
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntNullableWithAggregatesFilter | number | null
    _count?: NestedIntNullableFilter
    _avg?: NestedFloatNullableFilter
    _sum?: NestedIntNullableFilter
    _min?: NestedIntNullableFilter
    _max?: NestedIntNullableFilter
  }

  export type LocationRelationFilter = {
    is?: LocationWhereInput | null
    isNot?: LocationWhereInput | null
  }

  export type FileRelationFilter = {
    is?: FileWhereInput | null
    isNot?: FileWhereInput | null
  }

  export type TagOnFileListRelationFilter = {
    every?: TagOnFileWhereInput
    some?: TagOnFileWhereInput
    none?: TagOnFileWhereInput
  }

  export type TagOnFileOrderByRelationAggregateInput = {
    _count?: SortOrder
  }

  export type FileLocation_idStemNameExtensionCompoundUniqueInput = {
    location_id: number
    stem: string
    name: string
    extension: string
  }

  export type FileCountOrderByAggregateInput = {
    id?: SortOrder
    is_dir?: SortOrder
    location_id?: SortOrder
    stem?: SortOrder
    name?: SortOrder
    extension?: SortOrder
    quick_checksum?: SortOrder
    full_checksum?: SortOrder
    size_in_bytes?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    date_indexed?: SortOrder
    ipfs_id?: SortOrder
    parent_id?: SortOrder
  }

  export type FileAvgOrderByAggregateInput = {
    id?: SortOrder
    location_id?: SortOrder
    encryption?: SortOrder
    parent_id?: SortOrder
  }

  export type FileMaxOrderByAggregateInput = {
    id?: SortOrder
    is_dir?: SortOrder
    location_id?: SortOrder
    stem?: SortOrder
    name?: SortOrder
    extension?: SortOrder
    quick_checksum?: SortOrder
    full_checksum?: SortOrder
    size_in_bytes?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    date_indexed?: SortOrder
    ipfs_id?: SortOrder
    parent_id?: SortOrder
  }

  export type FileMinOrderByAggregateInput = {
    id?: SortOrder
    is_dir?: SortOrder
    location_id?: SortOrder
    stem?: SortOrder
    name?: SortOrder
    extension?: SortOrder
    quick_checksum?: SortOrder
    full_checksum?: SortOrder
    size_in_bytes?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    date_indexed?: SortOrder
    ipfs_id?: SortOrder
    parent_id?: SortOrder
  }

  export type FileSumOrderByAggregateInput = {
    id?: SortOrder
    location_id?: SortOrder
    encryption?: SortOrder
    parent_id?: SortOrder
  }

  export type TagCountOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
  }

  export type TagAvgOrderByAggregateInput = {
    id?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
  }

  export type TagMaxOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
  }

  export type TagMinOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
  }

  export type TagSumOrderByAggregateInput = {
    id?: SortOrder
    encryption?: SortOrder
    total_files?: SortOrder
    redundancy_goal?: SortOrder
  }

  export type TagRelationFilter = {
    is?: TagWhereInput
    isNot?: TagWhereInput
  }

  export type TagOnFileTag_idFile_idCompoundUniqueInput = {
    tag_id: number
    file_id: number
  }

  export type TagOnFileCountOrderByAggregateInput = {
    date_created?: SortOrder
    tag_id?: SortOrder
    file_id?: SortOrder
  }

  export type TagOnFileAvgOrderByAggregateInput = {
    tag_id?: SortOrder
    file_id?: SortOrder
  }

  export type TagOnFileMaxOrderByAggregateInput = {
    date_created?: SortOrder
    tag_id?: SortOrder
    file_id?: SortOrder
  }

  export type TagOnFileMinOrderByAggregateInput = {
    date_created?: SortOrder
    tag_id?: SortOrder
    file_id?: SortOrder
  }

  export type TagOnFileSumOrderByAggregateInput = {
    tag_id?: SortOrder
    file_id?: SortOrder
  }

  export type ClientRelationFilter = {
    is?: ClientWhereInput
    isNot?: ClientWhereInput
  }

  export type JobCountOrderByAggregateInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
  }

  export type JobAvgOrderByAggregateInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
  }

  export type JobMaxOrderByAggregateInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
  }

  export type JobMinOrderByAggregateInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
  }

  export type JobSumOrderByAggregateInput = {
    id?: SortOrder
    client_id?: SortOrder
    action?: SortOrder
    status?: SortOrder
    percentage_complete?: SortOrder
    task_count?: SortOrder
    completed_task_count?: SortOrder
  }

  export type LibraryRelationFilter = {
    is?: LibraryWhereInput | null
    isNot?: LibraryWhereInput | null
  }

  export type SpaceCountOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    libraryId?: SortOrder
  }

  export type SpaceAvgOrderByAggregateInput = {
    id?: SortOrder
    encryption?: SortOrder
    libraryId?: SortOrder
  }

  export type SpaceMaxOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    libraryId?: SortOrder
  }

  export type SpaceMinOrderByAggregateInput = {
    id?: SortOrder
    name?: SortOrder
    encryption?: SortOrder
    date_created?: SortOrder
    date_modified?: SortOrder
    libraryId?: SortOrder
  }

  export type SpaceSumOrderByAggregateInput = {
    id?: SortOrder
    encryption?: SortOrder
    libraryId?: SortOrder
  }

  export type StringFieldUpdateOperationsInput = {
    set?: string
  }

  export type IntFieldUpdateOperationsInput = {
    set?: number
    increment?: number
    decrement?: number
    multiply?: number
    divide?: number
  }

  export type DateTimeFieldUpdateOperationsInput = {
    set?: Date | string
  }

  export type SpaceCreateNestedManyWithoutLibraryInput = {
    create?: XOR<Enumerable<SpaceCreateWithoutLibraryInput>, Enumerable<SpaceUncheckedCreateWithoutLibraryInput>>
    connectOrCreate?: Enumerable<SpaceCreateOrConnectWithoutLibraryInput>
    connect?: Enumerable<SpaceWhereUniqueInput>
  }

  export type SpaceUncheckedCreateNestedManyWithoutLibraryInput = {
    create?: XOR<Enumerable<SpaceCreateWithoutLibraryInput>, Enumerable<SpaceUncheckedCreateWithoutLibraryInput>>
    connectOrCreate?: Enumerable<SpaceCreateOrConnectWithoutLibraryInput>
    connect?: Enumerable<SpaceWhereUniqueInput>
  }

  export type NullableStringFieldUpdateOperationsInput = {
    set?: string | null
  }

  export type BoolFieldUpdateOperationsInput = {
    set?: boolean
  }

  export type SpaceUpdateManyWithoutLibraryInput = {
    create?: XOR<Enumerable<SpaceCreateWithoutLibraryInput>, Enumerable<SpaceUncheckedCreateWithoutLibraryInput>>
    connectOrCreate?: Enumerable<SpaceCreateOrConnectWithoutLibraryInput>
    upsert?: Enumerable<SpaceUpsertWithWhereUniqueWithoutLibraryInput>
    set?: Enumerable<SpaceWhereUniqueInput>
    disconnect?: Enumerable<SpaceWhereUniqueInput>
    delete?: Enumerable<SpaceWhereUniqueInput>
    connect?: Enumerable<SpaceWhereUniqueInput>
    update?: Enumerable<SpaceUpdateWithWhereUniqueWithoutLibraryInput>
    updateMany?: Enumerable<SpaceUpdateManyWithWhereWithoutLibraryInput>
    deleteMany?: Enumerable<SpaceScalarWhereInput>
  }

  export type SpaceUncheckedUpdateManyWithoutLibraryInput = {
    create?: XOR<Enumerable<SpaceCreateWithoutLibraryInput>, Enumerable<SpaceUncheckedCreateWithoutLibraryInput>>
    connectOrCreate?: Enumerable<SpaceCreateOrConnectWithoutLibraryInput>
    upsert?: Enumerable<SpaceUpsertWithWhereUniqueWithoutLibraryInput>
    set?: Enumerable<SpaceWhereUniqueInput>
    disconnect?: Enumerable<SpaceWhereUniqueInput>
    delete?: Enumerable<SpaceWhereUniqueInput>
    connect?: Enumerable<SpaceWhereUniqueInput>
    update?: Enumerable<SpaceUpdateWithWhereUniqueWithoutLibraryInput>
    updateMany?: Enumerable<SpaceUpdateManyWithWhereWithoutLibraryInput>
    deleteMany?: Enumerable<SpaceScalarWhereInput>
  }

  export type JobCreateNestedManyWithoutClientsInput = {
    create?: XOR<Enumerable<JobCreateWithoutClientsInput>, Enumerable<JobUncheckedCreateWithoutClientsInput>>
    connectOrCreate?: Enumerable<JobCreateOrConnectWithoutClientsInput>
    connect?: Enumerable<JobWhereUniqueInput>
  }

  export type JobUncheckedCreateNestedManyWithoutClientsInput = {
    create?: XOR<Enumerable<JobCreateWithoutClientsInput>, Enumerable<JobUncheckedCreateWithoutClientsInput>>
    connectOrCreate?: Enumerable<JobCreateOrConnectWithoutClientsInput>
    connect?: Enumerable<JobWhereUniqueInput>
  }

  export type NullableBoolFieldUpdateOperationsInput = {
    set?: boolean | null
  }

  export type JobUpdateManyWithoutClientsInput = {
    create?: XOR<Enumerable<JobCreateWithoutClientsInput>, Enumerable<JobUncheckedCreateWithoutClientsInput>>
    connectOrCreate?: Enumerable<JobCreateOrConnectWithoutClientsInput>
    upsert?: Enumerable<JobUpsertWithWhereUniqueWithoutClientsInput>
    set?: Enumerable<JobWhereUniqueInput>
    disconnect?: Enumerable<JobWhereUniqueInput>
    delete?: Enumerable<JobWhereUniqueInput>
    connect?: Enumerable<JobWhereUniqueInput>
    update?: Enumerable<JobUpdateWithWhereUniqueWithoutClientsInput>
    updateMany?: Enumerable<JobUpdateManyWithWhereWithoutClientsInput>
    deleteMany?: Enumerable<JobScalarWhereInput>
  }

  export type JobUncheckedUpdateManyWithoutClientsInput = {
    create?: XOR<Enumerable<JobCreateWithoutClientsInput>, Enumerable<JobUncheckedCreateWithoutClientsInput>>
    connectOrCreate?: Enumerable<JobCreateOrConnectWithoutClientsInput>
    upsert?: Enumerable<JobUpsertWithWhereUniqueWithoutClientsInput>
    set?: Enumerable<JobWhereUniqueInput>
    disconnect?: Enumerable<JobWhereUniqueInput>
    delete?: Enumerable<JobWhereUniqueInput>
    connect?: Enumerable<JobWhereUniqueInput>
    update?: Enumerable<JobUpdateWithWhereUniqueWithoutClientsInput>
    updateMany?: Enumerable<JobUpdateManyWithWhereWithoutClientsInput>
    deleteMany?: Enumerable<JobScalarWhereInput>
  }

  export type FileCreateNestedManyWithoutLocationInput = {
    create?: XOR<Enumerable<FileCreateWithoutLocationInput>, Enumerable<FileUncheckedCreateWithoutLocationInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutLocationInput>
    connect?: Enumerable<FileWhereUniqueInput>
  }

  export type FileUncheckedCreateNestedManyWithoutLocationInput = {
    create?: XOR<Enumerable<FileCreateWithoutLocationInput>, Enumerable<FileUncheckedCreateWithoutLocationInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutLocationInput>
    connect?: Enumerable<FileWhereUniqueInput>
  }

  export type NullableIntFieldUpdateOperationsInput = {
    set?: number | null
    increment?: number
    decrement?: number
    multiply?: number
    divide?: number
  }

  export type FileUpdateManyWithoutLocationInput = {
    create?: XOR<Enumerable<FileCreateWithoutLocationInput>, Enumerable<FileUncheckedCreateWithoutLocationInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutLocationInput>
    upsert?: Enumerable<FileUpsertWithWhereUniqueWithoutLocationInput>
    set?: Enumerable<FileWhereUniqueInput>
    disconnect?: Enumerable<FileWhereUniqueInput>
    delete?: Enumerable<FileWhereUniqueInput>
    connect?: Enumerable<FileWhereUniqueInput>
    update?: Enumerable<FileUpdateWithWhereUniqueWithoutLocationInput>
    updateMany?: Enumerable<FileUpdateManyWithWhereWithoutLocationInput>
    deleteMany?: Enumerable<FileScalarWhereInput>
  }

  export type FileUncheckedUpdateManyWithoutLocationInput = {
    create?: XOR<Enumerable<FileCreateWithoutLocationInput>, Enumerable<FileUncheckedCreateWithoutLocationInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutLocationInput>
    upsert?: Enumerable<FileUpsertWithWhereUniqueWithoutLocationInput>
    set?: Enumerable<FileWhereUniqueInput>
    disconnect?: Enumerable<FileWhereUniqueInput>
    delete?: Enumerable<FileWhereUniqueInput>
    connect?: Enumerable<FileWhereUniqueInput>
    update?: Enumerable<FileUpdateWithWhereUniqueWithoutLocationInput>
    updateMany?: Enumerable<FileUpdateManyWithWhereWithoutLocationInput>
    deleteMany?: Enumerable<FileScalarWhereInput>
  }

  export type LocationCreateNestedOneWithoutFilesInput = {
    create?: XOR<LocationCreateWithoutFilesInput, LocationUncheckedCreateWithoutFilesInput>
    connectOrCreate?: LocationCreateOrConnectWithoutFilesInput
    connect?: LocationWhereUniqueInput
  }

  export type FileCreateNestedOneWithoutChildrenInput = {
    create?: XOR<FileCreateWithoutChildrenInput, FileUncheckedCreateWithoutChildrenInput>
    connectOrCreate?: FileCreateOrConnectWithoutChildrenInput
    connect?: FileWhereUniqueInput
  }

  export type FileCreateNestedManyWithoutParentInput = {
    create?: XOR<Enumerable<FileCreateWithoutParentInput>, Enumerable<FileUncheckedCreateWithoutParentInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutParentInput>
    connect?: Enumerable<FileWhereUniqueInput>
  }

  export type TagOnFileCreateNestedManyWithoutFileInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutFileInput>, Enumerable<TagOnFileUncheckedCreateWithoutFileInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutFileInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
  }

  export type FileUncheckedCreateNestedManyWithoutParentInput = {
    create?: XOR<Enumerable<FileCreateWithoutParentInput>, Enumerable<FileUncheckedCreateWithoutParentInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutParentInput>
    connect?: Enumerable<FileWhereUniqueInput>
  }

  export type TagOnFileUncheckedCreateNestedManyWithoutFileInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutFileInput>, Enumerable<TagOnFileUncheckedCreateWithoutFileInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutFileInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
  }

  export type LocationUpdateOneWithoutFilesInput = {
    create?: XOR<LocationCreateWithoutFilesInput, LocationUncheckedCreateWithoutFilesInput>
    connectOrCreate?: LocationCreateOrConnectWithoutFilesInput
    upsert?: LocationUpsertWithoutFilesInput
    disconnect?: boolean
    delete?: boolean
    connect?: LocationWhereUniqueInput
    update?: XOR<LocationUpdateWithoutFilesInput, LocationUncheckedUpdateWithoutFilesInput>
  }

  export type FileUpdateOneWithoutChildrenInput = {
    create?: XOR<FileCreateWithoutChildrenInput, FileUncheckedCreateWithoutChildrenInput>
    connectOrCreate?: FileCreateOrConnectWithoutChildrenInput
    upsert?: FileUpsertWithoutChildrenInput
    disconnect?: boolean
    delete?: boolean
    connect?: FileWhereUniqueInput
    update?: XOR<FileUpdateWithoutChildrenInput, FileUncheckedUpdateWithoutChildrenInput>
  }

  export type FileUpdateManyWithoutParentInput = {
    create?: XOR<Enumerable<FileCreateWithoutParentInput>, Enumerable<FileUncheckedCreateWithoutParentInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutParentInput>
    upsert?: Enumerable<FileUpsertWithWhereUniqueWithoutParentInput>
    set?: Enumerable<FileWhereUniqueInput>
    disconnect?: Enumerable<FileWhereUniqueInput>
    delete?: Enumerable<FileWhereUniqueInput>
    connect?: Enumerable<FileWhereUniqueInput>
    update?: Enumerable<FileUpdateWithWhereUniqueWithoutParentInput>
    updateMany?: Enumerable<FileUpdateManyWithWhereWithoutParentInput>
    deleteMany?: Enumerable<FileScalarWhereInput>
  }

  export type TagOnFileUpdateManyWithoutFileInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutFileInput>, Enumerable<TagOnFileUncheckedCreateWithoutFileInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutFileInput>
    upsert?: Enumerable<TagOnFileUpsertWithWhereUniqueWithoutFileInput>
    set?: Enumerable<TagOnFileWhereUniqueInput>
    disconnect?: Enumerable<TagOnFileWhereUniqueInput>
    delete?: Enumerable<TagOnFileWhereUniqueInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
    update?: Enumerable<TagOnFileUpdateWithWhereUniqueWithoutFileInput>
    updateMany?: Enumerable<TagOnFileUpdateManyWithWhereWithoutFileInput>
    deleteMany?: Enumerable<TagOnFileScalarWhereInput>
  }

  export type FileUncheckedUpdateManyWithoutParentInput = {
    create?: XOR<Enumerable<FileCreateWithoutParentInput>, Enumerable<FileUncheckedCreateWithoutParentInput>>
    connectOrCreate?: Enumerable<FileCreateOrConnectWithoutParentInput>
    upsert?: Enumerable<FileUpsertWithWhereUniqueWithoutParentInput>
    set?: Enumerable<FileWhereUniqueInput>
    disconnect?: Enumerable<FileWhereUniqueInput>
    delete?: Enumerable<FileWhereUniqueInput>
    connect?: Enumerable<FileWhereUniqueInput>
    update?: Enumerable<FileUpdateWithWhereUniqueWithoutParentInput>
    updateMany?: Enumerable<FileUpdateManyWithWhereWithoutParentInput>
    deleteMany?: Enumerable<FileScalarWhereInput>
  }

  export type TagOnFileUncheckedUpdateManyWithoutFileInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutFileInput>, Enumerable<TagOnFileUncheckedCreateWithoutFileInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutFileInput>
    upsert?: Enumerable<TagOnFileUpsertWithWhereUniqueWithoutFileInput>
    set?: Enumerable<TagOnFileWhereUniqueInput>
    disconnect?: Enumerable<TagOnFileWhereUniqueInput>
    delete?: Enumerable<TagOnFileWhereUniqueInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
    update?: Enumerable<TagOnFileUpdateWithWhereUniqueWithoutFileInput>
    updateMany?: Enumerable<TagOnFileUpdateManyWithWhereWithoutFileInput>
    deleteMany?: Enumerable<TagOnFileScalarWhereInput>
  }

  export type TagOnFileCreateNestedManyWithoutTagInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutTagInput>, Enumerable<TagOnFileUncheckedCreateWithoutTagInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutTagInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
  }

  export type TagOnFileUncheckedCreateNestedManyWithoutTagInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutTagInput>, Enumerable<TagOnFileUncheckedCreateWithoutTagInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutTagInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
  }

  export type TagOnFileUpdateManyWithoutTagInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutTagInput>, Enumerable<TagOnFileUncheckedCreateWithoutTagInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutTagInput>
    upsert?: Enumerable<TagOnFileUpsertWithWhereUniqueWithoutTagInput>
    set?: Enumerable<TagOnFileWhereUniqueInput>
    disconnect?: Enumerable<TagOnFileWhereUniqueInput>
    delete?: Enumerable<TagOnFileWhereUniqueInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
    update?: Enumerable<TagOnFileUpdateWithWhereUniqueWithoutTagInput>
    updateMany?: Enumerable<TagOnFileUpdateManyWithWhereWithoutTagInput>
    deleteMany?: Enumerable<TagOnFileScalarWhereInput>
  }

  export type TagOnFileUncheckedUpdateManyWithoutTagInput = {
    create?: XOR<Enumerable<TagOnFileCreateWithoutTagInput>, Enumerable<TagOnFileUncheckedCreateWithoutTagInput>>
    connectOrCreate?: Enumerable<TagOnFileCreateOrConnectWithoutTagInput>
    upsert?: Enumerable<TagOnFileUpsertWithWhereUniqueWithoutTagInput>
    set?: Enumerable<TagOnFileWhereUniqueInput>
    disconnect?: Enumerable<TagOnFileWhereUniqueInput>
    delete?: Enumerable<TagOnFileWhereUniqueInput>
    connect?: Enumerable<TagOnFileWhereUniqueInput>
    update?: Enumerable<TagOnFileUpdateWithWhereUniqueWithoutTagInput>
    updateMany?: Enumerable<TagOnFileUpdateManyWithWhereWithoutTagInput>
    deleteMany?: Enumerable<TagOnFileScalarWhereInput>
  }

  export type TagCreateNestedOneWithoutTag_filesInput = {
    create?: XOR<TagCreateWithoutTag_filesInput, TagUncheckedCreateWithoutTag_filesInput>
    connectOrCreate?: TagCreateOrConnectWithoutTag_filesInput
    connect?: TagWhereUniqueInput
  }

  export type FileCreateNestedOneWithoutFile_tagsInput = {
    create?: XOR<FileCreateWithoutFile_tagsInput, FileUncheckedCreateWithoutFile_tagsInput>
    connectOrCreate?: FileCreateOrConnectWithoutFile_tagsInput
    connect?: FileWhereUniqueInput
  }

  export type TagUpdateOneRequiredWithoutTag_filesInput = {
    create?: XOR<TagCreateWithoutTag_filesInput, TagUncheckedCreateWithoutTag_filesInput>
    connectOrCreate?: TagCreateOrConnectWithoutTag_filesInput
    upsert?: TagUpsertWithoutTag_filesInput
    connect?: TagWhereUniqueInput
    update?: XOR<TagUpdateWithoutTag_filesInput, TagUncheckedUpdateWithoutTag_filesInput>
  }

  export type FileUpdateOneRequiredWithoutFile_tagsInput = {
    create?: XOR<FileCreateWithoutFile_tagsInput, FileUncheckedCreateWithoutFile_tagsInput>
    connectOrCreate?: FileCreateOrConnectWithoutFile_tagsInput
    upsert?: FileUpsertWithoutFile_tagsInput
    connect?: FileWhereUniqueInput
    update?: XOR<FileUpdateWithoutFile_tagsInput, FileUncheckedUpdateWithoutFile_tagsInput>
  }

  export type ClientCreateNestedOneWithoutJobsInput = {
    create?: XOR<ClientCreateWithoutJobsInput, ClientUncheckedCreateWithoutJobsInput>
    connectOrCreate?: ClientCreateOrConnectWithoutJobsInput
    connect?: ClientWhereUniqueInput
  }

  export type ClientUpdateOneRequiredWithoutJobsInput = {
    create?: XOR<ClientCreateWithoutJobsInput, ClientUncheckedCreateWithoutJobsInput>
    connectOrCreate?: ClientCreateOrConnectWithoutJobsInput
    upsert?: ClientUpsertWithoutJobsInput
    connect?: ClientWhereUniqueInput
    update?: XOR<ClientUpdateWithoutJobsInput, ClientUncheckedUpdateWithoutJobsInput>
  }

  export type LibraryCreateNestedOneWithoutSpacesInput = {
    create?: XOR<LibraryCreateWithoutSpacesInput, LibraryUncheckedCreateWithoutSpacesInput>
    connectOrCreate?: LibraryCreateOrConnectWithoutSpacesInput
    connect?: LibraryWhereUniqueInput
  }

  export type LibraryUpdateOneWithoutSpacesInput = {
    create?: XOR<LibraryCreateWithoutSpacesInput, LibraryUncheckedCreateWithoutSpacesInput>
    connectOrCreate?: LibraryCreateOrConnectWithoutSpacesInput
    upsert?: LibraryUpsertWithoutSpacesInput
    disconnect?: boolean
    delete?: boolean
    connect?: LibraryWhereUniqueInput
    update?: XOR<LibraryUpdateWithoutSpacesInput, LibraryUncheckedUpdateWithoutSpacesInput>
  }

  export type NestedIntFilter = {
    equals?: number
    in?: Enumerable<number>
    notIn?: Enumerable<number>
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntFilter | number
  }

  export type NestedStringFilter = {
    equals?: string
    in?: Enumerable<string>
    notIn?: Enumerable<string>
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringFilter | string
  }

  export type NestedDateTimeFilter = {
    equals?: Date | string
    in?: Enumerable<Date> | Enumerable<string>
    notIn?: Enumerable<Date> | Enumerable<string>
    lt?: Date | string
    lte?: Date | string
    gt?: Date | string
    gte?: Date | string
    not?: NestedDateTimeFilter | Date | string
  }

  export type NestedIntWithAggregatesFilter = {
    equals?: number
    in?: Enumerable<number>
    notIn?: Enumerable<number>
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntWithAggregatesFilter | number
    _count?: NestedIntFilter
    _avg?: NestedFloatFilter
    _sum?: NestedIntFilter
    _min?: NestedIntFilter
    _max?: NestedIntFilter
  }

  export type NestedFloatFilter = {
    equals?: number
    in?: Enumerable<number>
    notIn?: Enumerable<number>
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedFloatFilter | number
  }

  export type NestedStringWithAggregatesFilter = {
    equals?: string
    in?: Enumerable<string>
    notIn?: Enumerable<string>
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringWithAggregatesFilter | string
    _count?: NestedIntFilter
    _min?: NestedStringFilter
    _max?: NestedStringFilter
  }

  export type NestedDateTimeWithAggregatesFilter = {
    equals?: Date | string
    in?: Enumerable<Date> | Enumerable<string>
    notIn?: Enumerable<Date> | Enumerable<string>
    lt?: Date | string
    lte?: Date | string
    gt?: Date | string
    gte?: Date | string
    not?: NestedDateTimeWithAggregatesFilter | Date | string
    _count?: NestedIntFilter
    _min?: NestedDateTimeFilter
    _max?: NestedDateTimeFilter
  }

  export type NestedStringNullableFilter = {
    equals?: string | null
    in?: Enumerable<string> | null
    notIn?: Enumerable<string> | null
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringNullableFilter | string | null
  }

  export type NestedBoolFilter = {
    equals?: boolean
    not?: NestedBoolFilter | boolean
  }

  export type NestedStringNullableWithAggregatesFilter = {
    equals?: string | null
    in?: Enumerable<string> | null
    notIn?: Enumerable<string> | null
    lt?: string
    lte?: string
    gt?: string
    gte?: string
    contains?: string
    startsWith?: string
    endsWith?: string
    not?: NestedStringNullableWithAggregatesFilter | string | null
    _count?: NestedIntNullableFilter
    _min?: NestedStringNullableFilter
    _max?: NestedStringNullableFilter
  }

  export type NestedIntNullableFilter = {
    equals?: number | null
    in?: Enumerable<number> | null
    notIn?: Enumerable<number> | null
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntNullableFilter | number | null
  }

  export type NestedBoolWithAggregatesFilter = {
    equals?: boolean
    not?: NestedBoolWithAggregatesFilter | boolean
    _count?: NestedIntFilter
    _min?: NestedBoolFilter
    _max?: NestedBoolFilter
  }

  export type NestedBoolNullableFilter = {
    equals?: boolean | null
    not?: NestedBoolNullableFilter | boolean | null
  }

  export type NestedBoolNullableWithAggregatesFilter = {
    equals?: boolean | null
    not?: NestedBoolNullableWithAggregatesFilter | boolean | null
    _count?: NestedIntNullableFilter
    _min?: NestedBoolNullableFilter
    _max?: NestedBoolNullableFilter
  }

  export type NestedIntNullableWithAggregatesFilter = {
    equals?: number | null
    in?: Enumerable<number> | null
    notIn?: Enumerable<number> | null
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedIntNullableWithAggregatesFilter | number | null
    _count?: NestedIntNullableFilter
    _avg?: NestedFloatNullableFilter
    _sum?: NestedIntNullableFilter
    _min?: NestedIntNullableFilter
    _max?: NestedIntNullableFilter
  }

  export type NestedFloatNullableFilter = {
    equals?: number | null
    in?: Enumerable<number> | null
    notIn?: Enumerable<number> | null
    lt?: number
    lte?: number
    gt?: number
    gte?: number
    not?: NestedFloatNullableFilter | number | null
  }

  export type SpaceCreateWithoutLibraryInput = {
    name: string
    encryption?: number | null
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type SpaceUncheckedCreateWithoutLibraryInput = {
    id?: number
    name: string
    encryption?: number | null
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type SpaceCreateOrConnectWithoutLibraryInput = {
    where: SpaceWhereUniqueInput
    create: XOR<SpaceCreateWithoutLibraryInput, SpaceUncheckedCreateWithoutLibraryInput>
  }

  export type SpaceUpsertWithWhereUniqueWithoutLibraryInput = {
    where: SpaceWhereUniqueInput
    update: XOR<SpaceUpdateWithoutLibraryInput, SpaceUncheckedUpdateWithoutLibraryInput>
    create: XOR<SpaceCreateWithoutLibraryInput, SpaceUncheckedCreateWithoutLibraryInput>
  }

  export type SpaceUpdateWithWhereUniqueWithoutLibraryInput = {
    where: SpaceWhereUniqueInput
    data: XOR<SpaceUpdateWithoutLibraryInput, SpaceUncheckedUpdateWithoutLibraryInput>
  }

  export type SpaceUpdateManyWithWhereWithoutLibraryInput = {
    where: SpaceScalarWhereInput
    data: XOR<SpaceUpdateManyMutationInput, SpaceUncheckedUpdateManyWithoutSpacesInput>
  }

  export type SpaceScalarWhereInput = {
    AND?: Enumerable<SpaceScalarWhereInput>
    OR?: Enumerable<SpaceScalarWhereInput>
    NOT?: Enumerable<SpaceScalarWhereInput>
    id?: IntFilter | number
    name?: StringFilter | string
    encryption?: IntNullableFilter | number | null
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
    libraryId?: IntNullableFilter | number | null
  }

  export type JobCreateWithoutClientsInput = {
    action: number
    status?: number
    percentage_complete?: number
    task_count?: number
    completed_task_count?: number
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type JobUncheckedCreateWithoutClientsInput = {
    id?: number
    action: number
    status?: number
    percentage_complete?: number
    task_count?: number
    completed_task_count?: number
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type JobCreateOrConnectWithoutClientsInput = {
    where: JobWhereUniqueInput
    create: XOR<JobCreateWithoutClientsInput, JobUncheckedCreateWithoutClientsInput>
  }

  export type JobUpsertWithWhereUniqueWithoutClientsInput = {
    where: JobWhereUniqueInput
    update: XOR<JobUpdateWithoutClientsInput, JobUncheckedUpdateWithoutClientsInput>
    create: XOR<JobCreateWithoutClientsInput, JobUncheckedCreateWithoutClientsInput>
  }

  export type JobUpdateWithWhereUniqueWithoutClientsInput = {
    where: JobWhereUniqueInput
    data: XOR<JobUpdateWithoutClientsInput, JobUncheckedUpdateWithoutClientsInput>
  }

  export type JobUpdateManyWithWhereWithoutClientsInput = {
    where: JobScalarWhereInput
    data: XOR<JobUpdateManyMutationInput, JobUncheckedUpdateManyWithoutJobsInput>
  }

  export type JobScalarWhereInput = {
    AND?: Enumerable<JobScalarWhereInput>
    OR?: Enumerable<JobScalarWhereInput>
    NOT?: Enumerable<JobScalarWhereInput>
    id?: IntFilter | number
    client_id?: IntFilter | number
    action?: IntFilter | number
    status?: IntFilter | number
    percentage_complete?: IntFilter | number
    task_count?: IntFilter | number
    completed_task_count?: IntFilter | number
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
  }

  export type FileCreateWithoutLocationInput = {
    is_dir?: boolean
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    parent?: FileCreateNestedOneWithoutChildrenInput
    children?: FileCreateNestedManyWithoutParentInput
    file_tags?: TagOnFileCreateNestedManyWithoutFileInput
  }

  export type FileUncheckedCreateWithoutLocationInput = {
    id?: number
    is_dir?: boolean
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    parent_id?: number | null
    children?: FileUncheckedCreateNestedManyWithoutParentInput
    file_tags?: TagOnFileUncheckedCreateNestedManyWithoutFileInput
  }

  export type FileCreateOrConnectWithoutLocationInput = {
    where: FileWhereUniqueInput
    create: XOR<FileCreateWithoutLocationInput, FileUncheckedCreateWithoutLocationInput>
  }

  export type FileUpsertWithWhereUniqueWithoutLocationInput = {
    where: FileWhereUniqueInput
    update: XOR<FileUpdateWithoutLocationInput, FileUncheckedUpdateWithoutLocationInput>
    create: XOR<FileCreateWithoutLocationInput, FileUncheckedCreateWithoutLocationInput>
  }

  export type FileUpdateWithWhereUniqueWithoutLocationInput = {
    where: FileWhereUniqueInput
    data: XOR<FileUpdateWithoutLocationInput, FileUncheckedUpdateWithoutLocationInput>
  }

  export type FileUpdateManyWithWhereWithoutLocationInput = {
    where: FileScalarWhereInput
    data: XOR<FileUpdateManyMutationInput, FileUncheckedUpdateManyWithoutFilesInput>
  }

  export type FileScalarWhereInput = {
    AND?: Enumerable<FileScalarWhereInput>
    OR?: Enumerable<FileScalarWhereInput>
    NOT?: Enumerable<FileScalarWhereInput>
    id?: IntFilter | number
    is_dir?: BoolFilter | boolean
    location_id?: IntFilter | number
    stem?: StringFilter | string
    name?: StringFilter | string
    extension?: StringNullableFilter | string | null
    quick_checksum?: StringNullableFilter | string | null
    full_checksum?: StringNullableFilter | string | null
    size_in_bytes?: StringFilter | string
    encryption?: IntFilter | number
    date_created?: DateTimeFilter | Date | string
    date_modified?: DateTimeFilter | Date | string
    date_indexed?: DateTimeFilter | Date | string
    ipfs_id?: StringNullableFilter | string | null
    parent_id?: IntNullableFilter | number | null
  }

  export type LocationCreateWithoutFilesInput = {
    name?: string | null
    path?: string | null
    total_capacity?: number | null
    available_capacity?: number | null
    is_removable?: boolean
    is_ejectable?: boolean
    is_root_filesystem?: boolean
    is_online?: boolean
    date_created?: Date | string
  }

  export type LocationUncheckedCreateWithoutFilesInput = {
    id?: number
    name?: string | null
    path?: string | null
    total_capacity?: number | null
    available_capacity?: number | null
    is_removable?: boolean
    is_ejectable?: boolean
    is_root_filesystem?: boolean
    is_online?: boolean
    date_created?: Date | string
  }

  export type LocationCreateOrConnectWithoutFilesInput = {
    where: LocationWhereUniqueInput
    create: XOR<LocationCreateWithoutFilesInput, LocationUncheckedCreateWithoutFilesInput>
  }

  export type FileCreateWithoutChildrenInput = {
    is_dir?: boolean
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    location?: LocationCreateNestedOneWithoutFilesInput
    parent?: FileCreateNestedOneWithoutChildrenInput
    file_tags?: TagOnFileCreateNestedManyWithoutFileInput
  }

  export type FileUncheckedCreateWithoutChildrenInput = {
    id?: number
    is_dir?: boolean
    location_id: number
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    parent_id?: number | null
    file_tags?: TagOnFileUncheckedCreateNestedManyWithoutFileInput
  }

  export type FileCreateOrConnectWithoutChildrenInput = {
    where: FileWhereUniqueInput
    create: XOR<FileCreateWithoutChildrenInput, FileUncheckedCreateWithoutChildrenInput>
  }

  export type FileCreateWithoutParentInput = {
    is_dir?: boolean
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    location?: LocationCreateNestedOneWithoutFilesInput
    children?: FileCreateNestedManyWithoutParentInput
    file_tags?: TagOnFileCreateNestedManyWithoutFileInput
  }

  export type FileUncheckedCreateWithoutParentInput = {
    id?: number
    is_dir?: boolean
    location_id: number
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    children?: FileUncheckedCreateNestedManyWithoutParentInput
    file_tags?: TagOnFileUncheckedCreateNestedManyWithoutFileInput
  }

  export type FileCreateOrConnectWithoutParentInput = {
    where: FileWhereUniqueInput
    create: XOR<FileCreateWithoutParentInput, FileUncheckedCreateWithoutParentInput>
  }

  export type TagOnFileCreateWithoutFileInput = {
    date_created?: Date | string
    tag: TagCreateNestedOneWithoutTag_filesInput
  }

  export type TagOnFileUncheckedCreateWithoutFileInput = {
    date_created?: Date | string
    tag_id: number
  }

  export type TagOnFileCreateOrConnectWithoutFileInput = {
    where: TagOnFileWhereUniqueInput
    create: XOR<TagOnFileCreateWithoutFileInput, TagOnFileUncheckedCreateWithoutFileInput>
  }

  export type LocationUpsertWithoutFilesInput = {
    update: XOR<LocationUpdateWithoutFilesInput, LocationUncheckedUpdateWithoutFilesInput>
    create: XOR<LocationCreateWithoutFilesInput, LocationUncheckedCreateWithoutFilesInput>
  }

  export type LocationUpdateWithoutFilesInput = {
    name?: NullableStringFieldUpdateOperationsInput | string | null
    path?: NullableStringFieldUpdateOperationsInput | string | null
    total_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    available_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    is_removable?: BoolFieldUpdateOperationsInput | boolean
    is_ejectable?: BoolFieldUpdateOperationsInput | boolean
    is_root_filesystem?: BoolFieldUpdateOperationsInput | boolean
    is_online?: BoolFieldUpdateOperationsInput | boolean
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type LocationUncheckedUpdateWithoutFilesInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: NullableStringFieldUpdateOperationsInput | string | null
    path?: NullableStringFieldUpdateOperationsInput | string | null
    total_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    available_capacity?: NullableIntFieldUpdateOperationsInput | number | null
    is_removable?: BoolFieldUpdateOperationsInput | boolean
    is_ejectable?: BoolFieldUpdateOperationsInput | boolean
    is_root_filesystem?: BoolFieldUpdateOperationsInput | boolean
    is_online?: BoolFieldUpdateOperationsInput | boolean
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type FileUpsertWithoutChildrenInput = {
    update: XOR<FileUpdateWithoutChildrenInput, FileUncheckedUpdateWithoutChildrenInput>
    create: XOR<FileCreateWithoutChildrenInput, FileUncheckedCreateWithoutChildrenInput>
  }

  export type FileUpdateWithoutChildrenInput = {
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    location?: LocationUpdateOneWithoutFilesInput
    parent?: FileUpdateOneWithoutChildrenInput
    file_tags?: TagOnFileUpdateManyWithoutFileInput
  }

  export type FileUncheckedUpdateWithoutChildrenInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    location_id?: IntFieldUpdateOperationsInput | number
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent_id?: NullableIntFieldUpdateOperationsInput | number | null
    file_tags?: TagOnFileUncheckedUpdateManyWithoutFileInput
  }

  export type FileUpsertWithWhereUniqueWithoutParentInput = {
    where: FileWhereUniqueInput
    update: XOR<FileUpdateWithoutParentInput, FileUncheckedUpdateWithoutParentInput>
    create: XOR<FileCreateWithoutParentInput, FileUncheckedCreateWithoutParentInput>
  }

  export type FileUpdateWithWhereUniqueWithoutParentInput = {
    where: FileWhereUniqueInput
    data: XOR<FileUpdateWithoutParentInput, FileUncheckedUpdateWithoutParentInput>
  }

  export type FileUpdateManyWithWhereWithoutParentInput = {
    where: FileScalarWhereInput
    data: XOR<FileUpdateManyMutationInput, FileUncheckedUpdateManyWithoutChildrenInput>
  }

  export type TagOnFileUpsertWithWhereUniqueWithoutFileInput = {
    where: TagOnFileWhereUniqueInput
    update: XOR<TagOnFileUpdateWithoutFileInput, TagOnFileUncheckedUpdateWithoutFileInput>
    create: XOR<TagOnFileCreateWithoutFileInput, TagOnFileUncheckedCreateWithoutFileInput>
  }

  export type TagOnFileUpdateWithWhereUniqueWithoutFileInput = {
    where: TagOnFileWhereUniqueInput
    data: XOR<TagOnFileUpdateWithoutFileInput, TagOnFileUncheckedUpdateWithoutFileInput>
  }

  export type TagOnFileUpdateManyWithWhereWithoutFileInput = {
    where: TagOnFileScalarWhereInput
    data: XOR<TagOnFileUpdateManyMutationInput, TagOnFileUncheckedUpdateManyWithoutFile_tagsInput>
  }

  export type TagOnFileScalarWhereInput = {
    AND?: Enumerable<TagOnFileScalarWhereInput>
    OR?: Enumerable<TagOnFileScalarWhereInput>
    NOT?: Enumerable<TagOnFileScalarWhereInput>
    date_created?: DateTimeFilter | Date | string
    tag_id?: IntFilter | number
    file_id?: IntFilter | number
  }

  export type TagOnFileCreateWithoutTagInput = {
    date_created?: Date | string
    file: FileCreateNestedOneWithoutFile_tagsInput
  }

  export type TagOnFileUncheckedCreateWithoutTagInput = {
    date_created?: Date | string
    file_id: number
  }

  export type TagOnFileCreateOrConnectWithoutTagInput = {
    where: TagOnFileWhereUniqueInput
    create: XOR<TagOnFileCreateWithoutTagInput, TagOnFileUncheckedCreateWithoutTagInput>
  }

  export type TagOnFileUpsertWithWhereUniqueWithoutTagInput = {
    where: TagOnFileWhereUniqueInput
    update: XOR<TagOnFileUpdateWithoutTagInput, TagOnFileUncheckedUpdateWithoutTagInput>
    create: XOR<TagOnFileCreateWithoutTagInput, TagOnFileUncheckedCreateWithoutTagInput>
  }

  export type TagOnFileUpdateWithWhereUniqueWithoutTagInput = {
    where: TagOnFileWhereUniqueInput
    data: XOR<TagOnFileUpdateWithoutTagInput, TagOnFileUncheckedUpdateWithoutTagInput>
  }

  export type TagOnFileUpdateManyWithWhereWithoutTagInput = {
    where: TagOnFileScalarWhereInput
    data: XOR<TagOnFileUpdateManyMutationInput, TagOnFileUncheckedUpdateManyWithoutTag_filesInput>
  }

  export type TagCreateWithoutTag_filesInput = {
    name?: string | null
    encryption?: number | null
    total_files?: number | null
    redundancy_goal?: number | null
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type TagUncheckedCreateWithoutTag_filesInput = {
    id?: number
    name?: string | null
    encryption?: number | null
    total_files?: number | null
    redundancy_goal?: number | null
    date_created?: Date | string
    date_modified?: Date | string
  }

  export type TagCreateOrConnectWithoutTag_filesInput = {
    where: TagWhereUniqueInput
    create: XOR<TagCreateWithoutTag_filesInput, TagUncheckedCreateWithoutTag_filesInput>
  }

  export type FileCreateWithoutFile_tagsInput = {
    is_dir?: boolean
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    location?: LocationCreateNestedOneWithoutFilesInput
    parent?: FileCreateNestedOneWithoutChildrenInput
    children?: FileCreateNestedManyWithoutParentInput
  }

  export type FileUncheckedCreateWithoutFile_tagsInput = {
    id?: number
    is_dir?: boolean
    location_id: number
    stem: string
    name: string
    extension?: string | null
    quick_checksum?: string | null
    full_checksum?: string | null
    size_in_bytes: string
    encryption?: number
    date_created?: Date | string
    date_modified?: Date | string
    date_indexed?: Date | string
    ipfs_id?: string | null
    parent_id?: number | null
    children?: FileUncheckedCreateNestedManyWithoutParentInput
  }

  export type FileCreateOrConnectWithoutFile_tagsInput = {
    where: FileWhereUniqueInput
    create: XOR<FileCreateWithoutFile_tagsInput, FileUncheckedCreateWithoutFile_tagsInput>
  }

  export type TagUpsertWithoutTag_filesInput = {
    update: XOR<TagUpdateWithoutTag_filesInput, TagUncheckedUpdateWithoutTag_filesInput>
    create: XOR<TagCreateWithoutTag_filesInput, TagUncheckedCreateWithoutTag_filesInput>
  }

  export type TagUpdateWithoutTag_filesInput = {
    name?: NullableStringFieldUpdateOperationsInput | string | null
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    total_files?: NullableIntFieldUpdateOperationsInput | number | null
    redundancy_goal?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type TagUncheckedUpdateWithoutTag_filesInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: NullableStringFieldUpdateOperationsInput | string | null
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    total_files?: NullableIntFieldUpdateOperationsInput | number | null
    redundancy_goal?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type FileUpsertWithoutFile_tagsInput = {
    update: XOR<FileUpdateWithoutFile_tagsInput, FileUncheckedUpdateWithoutFile_tagsInput>
    create: XOR<FileCreateWithoutFile_tagsInput, FileUncheckedCreateWithoutFile_tagsInput>
  }

  export type FileUpdateWithoutFile_tagsInput = {
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    location?: LocationUpdateOneWithoutFilesInput
    parent?: FileUpdateOneWithoutChildrenInput
    children?: FileUpdateManyWithoutParentInput
  }

  export type FileUncheckedUpdateWithoutFile_tagsInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    location_id?: IntFieldUpdateOperationsInput | number
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent_id?: NullableIntFieldUpdateOperationsInput | number | null
    children?: FileUncheckedUpdateManyWithoutParentInput
  }

  export type ClientCreateWithoutJobsInput = {
    uuid: string
    name: string
    platform?: number
    version?: string | null
    online?: boolean | null
    last_seen?: Date | string
    timezone?: string | null
    date_created?: Date | string
  }

  export type ClientUncheckedCreateWithoutJobsInput = {
    id?: number
    uuid: string
    name: string
    platform?: number
    version?: string | null
    online?: boolean | null
    last_seen?: Date | string
    timezone?: string | null
    date_created?: Date | string
  }

  export type ClientCreateOrConnectWithoutJobsInput = {
    where: ClientWhereUniqueInput
    create: XOR<ClientCreateWithoutJobsInput, ClientUncheckedCreateWithoutJobsInput>
  }

  export type ClientUpsertWithoutJobsInput = {
    update: XOR<ClientUpdateWithoutJobsInput, ClientUncheckedUpdateWithoutJobsInput>
    create: XOR<ClientCreateWithoutJobsInput, ClientUncheckedCreateWithoutJobsInput>
  }

  export type ClientUpdateWithoutJobsInput = {
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    platform?: IntFieldUpdateOperationsInput | number
    version?: NullableStringFieldUpdateOperationsInput | string | null
    online?: NullableBoolFieldUpdateOperationsInput | boolean | null
    last_seen?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type ClientUncheckedUpdateWithoutJobsInput = {
    id?: IntFieldUpdateOperationsInput | number
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    platform?: IntFieldUpdateOperationsInput | number
    version?: NullableStringFieldUpdateOperationsInput | string | null
    online?: NullableBoolFieldUpdateOperationsInput | boolean | null
    last_seen?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type LibraryCreateWithoutSpacesInput = {
    uuid: string
    name: string
    remote_id?: string | null
    is_primary?: boolean
    encryption?: number
    date_created?: Date | string
    timezone?: string | null
  }

  export type LibraryUncheckedCreateWithoutSpacesInput = {
    id?: number
    uuid: string
    name: string
    remote_id?: string | null
    is_primary?: boolean
    encryption?: number
    date_created?: Date | string
    timezone?: string | null
  }

  export type LibraryCreateOrConnectWithoutSpacesInput = {
    where: LibraryWhereUniqueInput
    create: XOR<LibraryCreateWithoutSpacesInput, LibraryUncheckedCreateWithoutSpacesInput>
  }

  export type LibraryUpsertWithoutSpacesInput = {
    update: XOR<LibraryUpdateWithoutSpacesInput, LibraryUncheckedUpdateWithoutSpacesInput>
    create: XOR<LibraryCreateWithoutSpacesInput, LibraryUncheckedCreateWithoutSpacesInput>
  }

  export type LibraryUpdateWithoutSpacesInput = {
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    remote_id?: NullableStringFieldUpdateOperationsInput | string | null
    is_primary?: BoolFieldUpdateOperationsInput | boolean
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
  }

  export type LibraryUncheckedUpdateWithoutSpacesInput = {
    id?: IntFieldUpdateOperationsInput | number
    uuid?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    remote_id?: NullableStringFieldUpdateOperationsInput | string | null
    is_primary?: BoolFieldUpdateOperationsInput | boolean
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    timezone?: NullableStringFieldUpdateOperationsInput | string | null
  }

  export type SpaceUpdateWithoutLibraryInput = {
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type SpaceUncheckedUpdateWithoutLibraryInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type SpaceUncheckedUpdateManyWithoutSpacesInput = {
    id?: IntFieldUpdateOperationsInput | number
    name?: StringFieldUpdateOperationsInput | string
    encryption?: NullableIntFieldUpdateOperationsInput | number | null
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type JobUpdateWithoutClientsInput = {
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type JobUncheckedUpdateWithoutClientsInput = {
    id?: IntFieldUpdateOperationsInput | number
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type JobUncheckedUpdateManyWithoutJobsInput = {
    id?: IntFieldUpdateOperationsInput | number
    action?: IntFieldUpdateOperationsInput | number
    status?: IntFieldUpdateOperationsInput | number
    percentage_complete?: IntFieldUpdateOperationsInput | number
    task_count?: IntFieldUpdateOperationsInput | number
    completed_task_count?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
  }

  export type FileUpdateWithoutLocationInput = {
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent?: FileUpdateOneWithoutChildrenInput
    children?: FileUpdateManyWithoutParentInput
    file_tags?: TagOnFileUpdateManyWithoutFileInput
  }

  export type FileUncheckedUpdateWithoutLocationInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent_id?: NullableIntFieldUpdateOperationsInput | number | null
    children?: FileUncheckedUpdateManyWithoutParentInput
    file_tags?: TagOnFileUncheckedUpdateManyWithoutFileInput
  }

  export type FileUncheckedUpdateManyWithoutFilesInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    parent_id?: NullableIntFieldUpdateOperationsInput | number | null
  }

  export type FileUpdateWithoutParentInput = {
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    location?: LocationUpdateOneWithoutFilesInput
    children?: FileUpdateManyWithoutParentInput
    file_tags?: TagOnFileUpdateManyWithoutFileInput
  }

  export type FileUncheckedUpdateWithoutParentInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    location_id?: IntFieldUpdateOperationsInput | number
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
    children?: FileUncheckedUpdateManyWithoutParentInput
    file_tags?: TagOnFileUncheckedUpdateManyWithoutFileInput
  }

  export type FileUncheckedUpdateManyWithoutChildrenInput = {
    id?: IntFieldUpdateOperationsInput | number
    is_dir?: BoolFieldUpdateOperationsInput | boolean
    location_id?: IntFieldUpdateOperationsInput | number
    stem?: StringFieldUpdateOperationsInput | string
    name?: StringFieldUpdateOperationsInput | string
    extension?: NullableStringFieldUpdateOperationsInput | string | null
    quick_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    full_checksum?: NullableStringFieldUpdateOperationsInput | string | null
    size_in_bytes?: StringFieldUpdateOperationsInput | string
    encryption?: IntFieldUpdateOperationsInput | number
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    date_modified?: DateTimeFieldUpdateOperationsInput | Date | string
    date_indexed?: DateTimeFieldUpdateOperationsInput | Date | string
    ipfs_id?: NullableStringFieldUpdateOperationsInput | string | null
  }

  export type TagOnFileUpdateWithoutFileInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    tag?: TagUpdateOneRequiredWithoutTag_filesInput
  }

  export type TagOnFileUncheckedUpdateWithoutFileInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    tag_id?: IntFieldUpdateOperationsInput | number
  }

  export type TagOnFileUncheckedUpdateManyWithoutFile_tagsInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    tag_id?: IntFieldUpdateOperationsInput | number
  }

  export type TagOnFileUpdateWithoutTagInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    file?: FileUpdateOneRequiredWithoutFile_tagsInput
  }

  export type TagOnFileUncheckedUpdateWithoutTagInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    file_id?: IntFieldUpdateOperationsInput | number
  }

  export type TagOnFileUncheckedUpdateManyWithoutTag_filesInput = {
    date_created?: DateTimeFieldUpdateOperationsInput | Date | string
    file_id?: IntFieldUpdateOperationsInput | number
  }



  /**
   * Batch Payload for updateMany & deleteMany & createMany
   */

  export type BatchPayload = {
    count: number
  }

  /**
   * DMMF
   */
  export const dmmf: runtime.DMMF.Document;
}