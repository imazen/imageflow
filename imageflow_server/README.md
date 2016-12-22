# imageflow_server

If you're compiling, use `cargo run --bin imageflow_server` instead of `imageflow_server`.

Currently we have 4 mount providers:
* ir4_http - ImageResizer4 compatible querystring API, pulling originals from a remote server
* ir4_local - ImageResizer4 compatible querystring API, pulling from disk
* static - static file server
* permacache_proxy - static file proxy with permanent caching (no invalidation, ever)

* `imageflow_server start --demo`
* `imageflow_server start --port 80 --data-dir=./imageflow_data --mount /ir4/local/:ir4_local:./img/ --mount /ir4/remote/:ir4_http:http:://remote.com/img/ --mount`
* `imageflow_server start --port 80 --data-dir=./imageflow_data  --mount /js/:static:./js --mount /proxy_asis/:permacache_proxy:http:://remote.com/static/:360`
* `imageflow_server diagnose --show-compilation-info`

http://localhost:3004/ir4/proxy_unsplash/photo-1422493757035-1e5e03968f95?width=600