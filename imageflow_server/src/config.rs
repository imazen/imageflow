
// use serde_toml
// Deserialize from TOML or from inline struct

// pub struct Hostname
//
// pub struct PerRequestLimits{
//    max_pixels_out: Option<i64>,
//    max_pixels_in: Option<i64>,
//    max_cpu_milliseconds: Option<i64>,
//    max_bitmap_ram_bytes: Option<i64>
// }
//
// pub struct ContentTypeRestrictions{
//    allow: Option<Vec<Mime>>,
//    deny: Option<Vec<Mime>>,
//    allow_extensions: Option<Vec<String>>,
//    deny_extensions: Option<Vec<String>>
// }
// pub struct SecurityPolicy{
//    per_request_limits: Option<PerRequestLimits>,
//    serve_content_types: Option<ContentTypeRestrictions>,
//    proxy_content_types: Option<ContentTypeRestrictions>,
//    force_image_recoding: Option<bool>
// }
//
// pub enum BlobSource{
//    Directory(String),
//    HttpServer(String),
//    //TODO: Azure and S3 blob backend
// }
//
// pub enum InternalCachingStrategy{
//    PubSubAndPermaPyramid,
//    TrackStatsAndPermaPyramid,
//    OpportunistPermaPyramid,
//    PubSubToInvalidate,
//    OpportunistPubSubEtagCheck,
//
// }
// pub struct CacheControlPolicy{
// //How do we set etag/last modified/expires/maxage?
// }
//
// pub struct BaseConfig{
//    //Security defaults
//    pub security: Option<SecurityPolicy>,
//    //May also want to filter by hostnames or ports for heavy multi-tenanting
//    pub cache_control: Option<CacheControlPolicy>
// }
//
// pub enum Frontend{
//    ImageResizer4Compatible,
//    Flow0
// }
// pub struct MountPath {
//    //Where we get originals from
//    pub source: BlobSource,
//    //The virtual path for which we handle sub-requests.
//    pub prefix: String,
//    //Customize security
//    pub security: Option<SecurityPolicy>,
//    //May also want to filter by hostnames or ports for heavy multi-tenanting
//    pub cache_control: Option<CacheControlPolicy>,
//
//    pub api: Frontend
// }