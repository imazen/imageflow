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

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `parallel "$HOME/bin/imageflow_tool v0.1/ir4 --in {} --out ../bench_out/{.}_200x200.jpg --command width=200&height=200&quality=90" ::: *.jpg` | 479.7 ± 51.2 | 411.5 | 571.2 | 1.00 |
| `parallel "vipsthumbnail --linear --size=200x200  --output=../bench_out/{.}_vips_200x200.jpg[Q=90] {}" ::: *.jpg` | 1212.4 ± 46.1 | 1149.9 | 1284.7 | 2.53 ± 0.29 |
| `parallel "convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_200x200.jpg" ::: *.jpg` | 8821.8 ± 171.6 | 8674.9 | 9234.4 | 18.39 ± 2.00 |
| `parallel "convert {} -set colorspace sRGB -colorspace RGB -filter  Mitchell -distort Resize 200x200  -colorspace sRGB -quality 90 ../bench_out/{.}_magick_ideal_200x200.jpg" ::: *.jpg` | 11795.0 ± 299.6 | 11246.9 | 12269.7 | 24.59 ± 2.70 |

