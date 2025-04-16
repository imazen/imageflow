---
description: 
globs: *.rs,*.md
alwaysApply: false
---
It's important to reference the files below when touching anything to do with the Imageflow API, Instructions, querystring commands, RIAPI, or Ir4Commands.

To work effectively with Imageflow, you should understand that Imageflow's main (98%) usage path is via querystring commands, like ?w=20&h=20. 

This syntax is backwards compatible with ImageResizer 1,2,3,4. There are around 100 commands, defined in @/imageflow_riapi/src/ir4/parsing.rs and @/imageflow_riapi/src/srcset.rs, and are transformed to JSON with the logic in @/imageflow_riapi/src/encoder.rs, @/imageflow_riapi/src/layout.rs, @/imageflow_riapi/src/mod.rs, and @/imageflow_riapi/sizing.rs. The query is not expanded to JSON all at once, part happens before the image is decoded, and part happens afterwards. 

These commands are partially documented in @/docs/src/querystring/encoding.md, @/docs/src/querystring/examples.md, @/docs/src/querystring/filters.md, @/docs/src/querystring/introduction.md, @/docs/src/querystring/transforms.md, and @/docs/src/querystring/rfc-qp.md, and we need to keep them up-to-date.

We want to expand unit tests to cover common cases, so that we can prove our documented explanations are correct, and expand that documentation with concrete examples.

Users typically interact via Imageflow.Net, a C# wrapper over the JSON FFI interface (see [ffi.mdc](mdc:.cursor/rules/ffi.mdc)), or via Imageflow Server (https://github.com/imazen/imageflow-server), which offers an HTTP API, and lets you map image sources to virtual paths and add querystrings to modify them on-the-fly (thus our focus on performance.) There are other bindings for go/node/elixr/etc. 

The current project also contains the imageflow_tool command-line tool, which can be used with JSON or like `imageflow_tool v1/querystring --command width=60&height=40&mode=max&format=jpg --in 100x100.jpg --out out4.jpg`, which is implemented in @/imageflow_tool/src/lib.rs, @/imageflow_tool/src/self_test.rs, and @/imageflow_tool/src/cmd_build.rs, 

NB: When working on @/imageflow_riapi/src/parsing.rs, make sure both serialization and deserialization are fully tested, and that new querystring keys are added to that allowlist array. 

When making examples for docs or schema, we try to use good, stunning photographs when we can, via our RIAPI compliant Imageflow Server deployment at https://i.zr.io/ri/i/glacier.jpg?w=300

Here are some base URLs, with their intrinsic sizes and aspect ratios:

https://i.zr.io/ri/i/above-trees.jpg 2400x3200 3:4
https://i.zr.io/ri/i/above-trees_orig.jpg 6048x8064 3:4
https://i.zr.io/ri/i/glacier.jpg 2400x3600 2:3
https://i.zr.io/ri/i/horse.jpg 2400x3600 2:3
https://i.zr.io/ri/i/komodo-island.jpg 2400x3226 1200:1613
https://i.zr.io/ri/i/lake-town.jpg 2400x3598 1200:1799
https://i.zr.io/ri/i/pearl-tower.jpg 2400x3600 2:3
https://i.zr.io/ri/i/sahara.jpg 2400x1600 3:2
https://i.zr.io/ri/i/santorini.jpg 2400x3600 2:3
https://i.zr.io/ri/i/snowy-trees.jpg 2400x1797 800:599
https://i.zr.io/ri/i/woman.jpg 2400x3600 2:3