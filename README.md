imageflow - Real-time image processing for the web.
=========

**The Problem**: Image processing is ubiquitously required. All popular CMSes, many CDNs, and most asset pipelines implement at least image scaling and coding. The need for mobile-friendly websites (and consequently responsive images) make manual asset creation methods time-prohibitive. Batch asset generation is error-prone, highly latent (affecting UX), and severely restrics web development agility. 

Existing implementations are either (a) incorrect, causing visual artifacts or (b) so slow that they've created industry cargo-cult assumptions about `architectural needs`; I.e, *always* use a queue and workers, because we can gzip large files on the fly but not jpeg encode them (which makes no sense from big O standpoint). 

In addition, **all are insecure**, having either failed to consider that image data is frequently malicious or having complexity and dependencies that are impossible to audit. The most commonly deployed implementations have dozens of trivially discovered vulnerabilities and buffer overflows.

**The solution**: Create a library that is safe for use with malicious data, and says NO to any of the following

* Operations that do not have predicitable resource (RAM/CPU) consumption. 
* Operations that cannot be performed in under 100ms on a 16MP jpeg, on a single i7 core. 
* Operations that undermine security in any way.
* Dependencies that have a questionable security track-record. LibTiff, etc. 
* Optimizations that cause incorrect results (such as failing to perform color-correction, or scaling in the sRGB space instead of linear). (Or using 8-bit instead of 14-bit per channel when )
* Abstractions that prevent major optimizations (over 30%). Some of the most common (enforced codec agnosticism) can prevent ~3000% reductions in cost. 

### Simplifying assumptions

* 32-bit sRGB is our 'common memory format'. To interoperate with other libraries, we must support endian-specific layout. (BGRA on little-endian, ARGB on big-endian). Endian-agnostic layout may also be required by some libraries; this needs to be investigated.
* We use 128-bit floating point (BGRA, linear, premultiplied) for operations that blend pixels. (Linear RGB in 32-bits causes severe truncation).
* The uncompressed 32-bit image can fit in RAM. If it can't, we don't do it. This is for web, and if a server can't allocate enough space for an bitmap, neither can a mobile phone. Also; at least 1 matrix transposition is required for downsampling an image, and this essentially requires it all to be in memory.
* We support jpeg, gif, and png natively. All other codecs are plugins. 








