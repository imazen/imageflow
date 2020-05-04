# Introduction

* Imageflow can be used as a library (libimageflow or [imageflow-dotnet](https://github.com/imazen/imageflow-dotnet))
* Imageflow can be used as a command-line tool for scripting ([imageflow_tool](imageflow_tool.md))
* Imageflow can be used as an HTTP server ([imageflow_server](imageflow_server.md))

All share support for the querystring API ([RIAPI](querystring.md)). 
libimageflow, imageflow-dotnet, and imageflow_tool currently support the [JSON API](json.md).

The querystring API is much simpler, but the JSON API can compose multiple images or generate multiple image 
versions in a single job. You can also use the querystring API from within the JSON API. 



