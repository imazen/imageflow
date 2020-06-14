# Imageflow Benchmarking Image based on Ubuntu 20.04

We want to test the throughput of various image processing tools, i.e, how many images they can process per second. 

To do this, we'll use `hyperfine` and `parallel` to test each tool with 32 copies of a [17-megapixel jpeg](https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg) and see which completes first in wall time.

We're measuring correct scaling, so we want the Q16 version of imagemagick (the default), and we're scaling in linear light. 
(Blending pixels in non-linear light will destroy image highlights). 

See `bench.sh` for the exact commands in use. 

To run these on your own machine, `docker run imazen/imageflow_bench_ubuntu20`.
 
You can build your own copy of the docker image by running `docker build . -t imazen/imageflow_bench_ubuntu20` inside this directory. 
 
To open bash and edit the benchmark run `docker run -it --entrypoint /bin/bash imazen/imageflow_bench_ubuntu20`

Please submit pull requests for any improvements!


### Scaling 32 17MP jpegs down to 200x200px
| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `parallel "$HOME/bin/imageflow_tool v1/querystring --in {} --out ../bench_out/{.}_200x200.jpg --command width=200&height=200&quality=90" ::: *.jpg` | 373.9 ± 30.8 | 310.0 | 409.8 | 1.00 |
| `parallel "vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg[Q=90] {}" ::: *.jpg` | 1002.8 ± 77.2 | 872.0 | 1123.7 | 2.68 ± 0.30 |
| `parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_200x200.jpg" ::: *.jpg` | 6555.1 ± 147.6 | 6352.6 | 6794.0 | 17.53 ± 1.50 |
| `parallel "convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_200x200.jpg" ::: *.jpg` | 8657.5 ± 215.2 | 8408.4 | 9105.4 | 23.15 ± 1.99 |

### Scaling 32 17MP jpegs down to 2000x2000px

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `parallel "$HOME/bin/imageflow_tool v1/querystring --in {} --out ../bench_out/{.}_2000x2000.jpg --command width=2000&height=2000&quality=90" ::: *.jpg` | 1.014 ± 0.092 | 0.892 | 1.172 | 1.00 |
| `parallel "vipsthumbnail --linear --size=2000x2000  --output=../bench_out/{.}_vips_2000x2000.jpg[Q=90] {}" ::: *.jpg` | 2.405 ± 0.019 | 2.373 | 2.449 | 2.37 ± 0.22 |
| `parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_2000x2000.jpg" ::: *.jpg` | 7.714 ± 0.160 | 7.493 | 7.918 | 7.61 ± 0.71 |
| `parallel "convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 2000x2000  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_2000x2000.jpg" ::: *.jpg` | 12.056 ± 0.427 | 11.552 | 12.759 | 11.89 ± 1.16 |
