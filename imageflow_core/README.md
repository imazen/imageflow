# imageflow_core

This will contain all functionality included in libimageflow

Dependencies
* libimageflow_c

Build using ./buildc.sh 

## Scope.sh

This uses resamplescope to plot the resampling functions of imageflow and imageworsener

run ./scope.sh

When complete, the folder `./scope` will be opened for you to compare the plots.

## Compare.rb

This system will test 8 different images, scaled to 3 different sizes, with 11 different filters, with and without gamma correction, and compare the results with `imagemagick`, `imageworsener`, and `flow-proto`, both using DSSIM and by generating an interactive web page for subjective analysis. 

The most dissimilar files will be listed first.

Open ./compare/index.html in your browser when complete. 

## Updating Imageflow results

1.  Run `./compare_reset_flow_images.sh` to invalidate all imageflow-generated results. 
2. delete flow-proto1
3. Run `./install_tools.sh`
4. Run  `./compare.rb`
