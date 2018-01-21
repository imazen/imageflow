# Redis usage

## Configuration

* volatile-ttl allows us to confidently store metadata for remote objects, but also utilize leftover space for byte or bitmap data caching
* Persistence required, but interval is less crucial. 
* maxmemory-policy to noeviction will block writes instead of exhausting resources.

Partitioning across Redis servers needs to happen by an inexpensively-computed key. For RIAPI requests this could be the 'primary image' base URL. For JSON requests this could be the first listed resource. Duplicates would happen for multi-image sources.

## Schema:

### Metadata-caching mutable-content URIs

```
Key: uri_hash
Value (etag, last_modified, last_revalidation, req_flag, auth_fla, inaccessible_after, failure_handling)
```

* last_revalidation - the last time we checked remote metadata
* req_flag: Compact identification of permutations of cache-control and conditional-validation headers https://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html#sec14.9.4
* auth_flag: Identifier of authentication method required (may require post-processing the URL)
* inaccessible_after: Some URLs are designed to expire. We just want to delete these (and things which use them) instead of retrying. 
* failure_handling: Id and state machine for dealing with transport and unexpected response codes

To get an "immutable hash", one hashes (uri_hash, etag, and last_modified)


### Revalidation queue

We need to limit the queue size (LTRIM helps here). Can be very high number, but dropping revalidations is OK.

items: (uri_hash, revalidate_age)

It's probably better to use the keyspace for set deduplication. revalidate_age make this lua-scriptable.

On invalidation: enqueue cache cleanup for the old "immutable hash" and all things which depend on it.

### Invalidation of dependencies

We maintain a list of dependencies to permit invalidation. Whenever we insert an item we append it to the set maintained by each thing it depends upon.

immutable_hash_dependencies: {set of immutable_hashes}


## Variants of originals

Like dependency invalidation, we maintain a set of variations to original images. The acceptance criteria for variants in this list might be: "quality > 70 if jpeg, maxwidth/maxheight/autorotate only, no IDCT scaling". 

hash: {variants (hashes)}

## Blob hashes

* width
* height
* format
* encoding settings (subsampling affects variant selection)

* byte_length
* acceptable_variant
* net_cost
* gross_cost
* LRU state machine

Blob hashes don't have to actually point to content. They can be accumulating usage before actually uploading.

//When writing, we also need to persist (to the storage container) all the dependencies of this blob

//What about caching parsed image dimensions but not the bytes? Would this speed up fail-fast?

## Copies
hash_copies: List
* storage container
* storage id
* expected latency (but from different servers?)


# Multiple remote stores.

We may blend local disk, network disk, remote Azure, and remote S3. How these are selected depends on the cache admission protocol. 

Consequences:
* additional field or separate redis databases?
* Should each store have its own LRU/eviction? RANDOMKEY is per database. Or is filtering by key pattern better?
* Should we support moves between stores?
* redis databases are integers. Default limit is 16.


# Manual LRU

We can maintain a pool of eviction candidates. We randomly sample the keyspace 10 times to select new candidates, then compare them to the existing pool. We drop less promising candidates and add better ones.

For URLs with inaccessible_after, we drop them immediately.

We then drop enough candidates from the pool to meet our byte & count quota. We do this by deleting them from the keyspace and adding them to a deletion queue. After deletion we re-verify they weren't added to the keyspace. If they were, we log info about churn. 



# Recovery

Blob stores must have a way to list blobs - even when there are millions. A hex folder tree may be required.
When we store a blob, we should store everything needed to recreate it associated metadata, in a sidecar JSON file (or blob metadata).

# Cache admission strategies

Metadata always goes in when encountered. Blobs have admission criteria

* Second request?
* Sliding scale based on recent evictions?
* Request/computation cost vs. storage container retrieval cost




# Strategies

* Output cache (potentially allowing redirects)
* Input cache (for high-latency sources)


Warm cache with queue of originals expected to access (or with a previous log).
Warm cache externally via HTTP HEAD requests to exposed APIs.
Pyramid images that meet admission criteria
OpportunisticPyramid 
Force recursive revalidation for specific responses


    PubSubAndPermaPyramid,
    TrackStatsAndPermaPyramid,
    OpportunistPermaPyramid,
    PubSubToInvalidate,
    OpportunistPubSubEtagCheck,
