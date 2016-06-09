cargo build --release
cp target/release/flow-proto1 .

convert --version
flow-proto1 --version

wget -nc  https://s3-us-west-2.amazonaws.com/imageflow-resources/test_inputs/u1.jpg
cp u1.jpg c1.jpg
cp u1.jpg c2.jpg
cp u1.jpg c3.jpg
cp u1.jpg c4.jpg
cp u1.jpg c5.jpg
cp u1.jpg c6.jpg
cp u1.jpg c7.jpg

rm -rf bench_out
mkdir bench_out

echo 'Using imageflow to thumbnail'
time parallel './flow-proto1 -i {} -o bench_out/{.}_200x200.jpg -w 200 -h 200' ::: *.jpg
echo
echo 
echo 'Using ImageMagick to thumbnail'
time parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 200x200  -colorspace sRGB bench_out/{.}_magick_200x200.jpg' ::: *.jpg

echo
echo
echo 'Using imageflow to create 2000px versions'
time parallel './flow-proto1 -i {} -o bench_out/{.}_2000x2000.jpg -w 2000 -h 2000' ::: *.jpg
echo
echo
echo 'Using ImageMagick to create 2000px versions'
time parallel 'convert {} -set colorspace sRGB -colorspace RGB -filter Robidoux -resize 2000x2000  -colorspace sRGB bench_out/{.}_magick_2000x2000.jpg' ::: *.jpg

echo
echo
echo 'Using imageflow wrong'
time parallel './flow-proto1 -i {} --incorrectgamma -o bench_out/{.}_200x200.jpg -w 200 -h 200' ::: *.jpg
echo
echo
echo 'Using ImageMagick wrong'
time parallel 'convert {} -filter Robidoux -resize 200x200 bench_out/{.}_magick_200x200.jpg' ::: *.jpg
