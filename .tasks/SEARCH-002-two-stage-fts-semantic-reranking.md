---
id: SEARCH-002
title: Two-Stage FTS5 + Semantic Re-ranking
status: To Do
assignee: james
parent: SEARCH-000
priority: High
tags: [search, fts, semantic-search, ai]
whitepaper: Section 4.7
---

## Description

Implement the two-stage search process that combines fast FTS5 keyword filtering with more computationally expensive semantic re-ranking. This approach provides a good balance between performance and relevance.

## Implementation Steps

1.  Integrate FTS5 into the SQLite database for fast full-text search.
2.  Implement the first stage of the search, which uses FTS5 to quickly narrow down the search space.
3.  Implement the second stage, which takes the results from the first stage and re-ranks them based on semantic similarity.
4.  Develop the logic to combine the results from both stages into a single, relevance-ranked list.

## Acceptance Criteria

- [ ] The system can perform fast keyword searches using FTS5.
- [ ] The system can re-rank search results based on semantic similarity.
- [ ] The two-stage search process is implemented and functional.
