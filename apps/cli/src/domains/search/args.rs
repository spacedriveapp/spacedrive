use clap::Args;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use sd_core::ops::search::input::{
    FileSearchInput, SearchScope, SearchMode, SearchFilters, TagFilter, 
    DateRangeFilter, DateField, SizeRangeFilter, ContentType, SortOptions, 
    SortField, SortDirection, PaginationOptions
};

#[derive(Args, Debug)]
pub struct FileSearchArgs {
    /// Search query
    pub query: String,
    
    /// Search mode
    #[arg(long, value_enum, default_value = "normal")]
    pub mode: SearchModeArg,
    
    /// File type filter (can be specified multiple times)
    #[arg(long)]
    pub file_type: Option<Vec<String>>,
    
    /// Tag filter (can be specified multiple times)
    #[arg(long)]
    pub tags: Option<Vec<Uuid>>,
    
    /// Exclude tags (can be specified multiple times)
    #[arg(long)]
    pub exclude_tags: Option<Vec<Uuid>>,
    
    /// Location filter
    #[arg(long)]
    pub location: Option<Uuid>,
    
    /// Date field for filtering
    #[arg(long, value_enum, default_value = "modified")]
    pub date_field: DateFieldArg,
    
    /// Start date for filtering (ISO format)
    #[arg(long)]
    pub date_start: Option<DateTime<Utc>>,
    
    /// End date for filtering (ISO format)
    #[arg(long)]
    pub date_end: Option<DateTime<Utc>>,
    
    /// Minimum file size in bytes
    #[arg(long)]
    pub min_size: Option<u64>,
    
    /// Maximum file size in bytes
    #[arg(long)]
    pub max_size: Option<u64>,
    
    /// Content type filter
    #[arg(long, value_enum)]
    pub content_type: Option<Vec<ContentTypeArg>>,
    
    /// Sort field
    #[arg(long, value_enum, default_value = "relevance")]
    pub sort_field: SortFieldArg,
    
    /// Sort direction
    #[arg(long, value_enum, default_value = "desc")]
    pub sort_direction: SortDirectionArg,
    
    /// Limit number of results
    #[arg(long, default_value = "50")]
    pub limit: u32,
    
    /// Offset for pagination
    #[arg(long, default_value = "0")]
    pub offset: u32,
    
    /// Include hidden files
    #[arg(long)]
    pub include_hidden: bool,
    
    /// Include archived files
    #[arg(long)]
    pub include_archived: bool,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SearchModeArg {
    Fast,
    Normal,
    Full,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum DateFieldArg {
    Created,
    Modified,
    Accessed,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ContentTypeArg {
    Image,
    Video,
    Audio,
    Document,
    Code,
    Text,
    Archive,
    Other,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SortFieldArg {
    Relevance,
    Name,
    Size,
    Modified,
    Created,
}

#[derive(clap::ValueEnum, Debug, Clone)]
pub enum SortDirectionArg {
    Asc,
    Desc,
}

impl From<FileSearchArgs> for FileSearchInput {
    fn from(args: FileSearchArgs) -> Self {
        let mode = match args.mode {
            SearchModeArg::Fast => SearchMode::Fast,
            SearchModeArg::Normal => SearchMode::Normal,
            SearchModeArg::Full => SearchMode::Full,
        };
        
        let scope = if let Some(location_id) = args.location {
            SearchScope::Location { location_id }
        } else {
            SearchScope::Library
        };
        
        let filters = SearchFilters {
            file_types: args.file_type,
            tags: if args.tags.is_some() || args.exclude_tags.is_some() {
                Some(TagFilter {
                    include: args.tags.unwrap_or_default(),
                    exclude: args.exclude_tags.unwrap_or_default(),
                })
            } else {
                None
            },
            date_range: if args.date_start.is_some() || args.date_end.is_some() {
                Some(DateRangeFilter {
                    field: match args.date_field {
                        DateFieldArg::Created => DateField::CreatedAt,
                        DateFieldArg::Modified => DateField::ModifiedAt,
                        DateFieldArg::Accessed => DateField::AccessedAt,
                    },
                    start: args.date_start,
                    end: args.date_end,
                })
            } else {
                None
            },
            size_range: if args.min_size.is_some() || args.max_size.is_some() {
                Some(SizeRangeFilter {
                    min: args.min_size,
                    max: args.max_size,
                })
            } else {
                None
            },
            locations: None, // Not used in CLI for now
            content_types: args.content_type.map(|types| {
                types.into_iter().map(|ct| match ct {
                    ContentTypeArg::Image => ContentType::Image,
                    ContentTypeArg::Video => ContentType::Video,
                    ContentTypeArg::Audio => ContentType::Audio,
                    ContentTypeArg::Document => ContentType::Document,
                    ContentTypeArg::Code => ContentType::Code,
                    ContentTypeArg::Text => ContentType::Text,
                    ContentTypeArg::Archive => ContentType::Archive,
                    ContentTypeArg::Other => ContentType::Other,
                }).collect()
            }),
            include_hidden: Some(args.include_hidden),
            include_archived: Some(args.include_archived),
        };
        
        let sort = SortOptions {
            field: match args.sort_field {
                SortFieldArg::Relevance => SortField::Relevance,
                SortFieldArg::Name => SortField::Name,
                SortFieldArg::Size => SortField::Size,
                SortFieldArg::Modified => SortField::ModifiedAt,
                SortFieldArg::Created => SortField::CreatedAt,
            },
            direction: match args.sort_direction {
                SortDirectionArg::Asc => SortDirection::Asc,
                SortDirectionArg::Desc => SortDirection::Desc,
            },
        };
        
        let pagination = PaginationOptions {
            limit: args.limit,
            offset: args.offset,
        };
        
        Self {
            query: args.query,
            scope,
            mode,
            filters,
            sort,
            pagination,
        }
    }
}