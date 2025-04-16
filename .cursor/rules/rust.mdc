---
description: 
globs: 
alwaysApply: true
---
When working on imageflow, we want to preserve comments in the code - ALWAYS - since they help us remember corner cases. Always repeat them back out into the new code. And always load /imageflow_types/src/lib.rs into memory, since we use it in every rust file.

Our primary interfaces users see are the RIAPI (querystring commands) @riapi.mdc and the JSON API @json-api.mdc, both accessed via @ffi.mdc + Imageflow.Net and Imageflow Server.

We want to keep our gitbook in docs/src up to date.

When creating an Err() in imageflow_core, use the nerror!(ErrorKind:*, "format {}", args) macro (format/args optional), which embeds line/file data and creates a FlowError struct. 
We can map other error types to it stringly, via .map_err(|e| nerror!(ErrorKind::InvalidArgument, "Parse error: {}", e))?;
We add to the artificial error call stack with .map_err(|e| e.at(here!()))?.
See @errors.rs for the available ErrorKinds. 

Example: Err(nerror!(ErrorKind::InvalidOperation, "LibPNG decoder disposed before call to read_frame"))