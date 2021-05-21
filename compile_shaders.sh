#!/bin/sh

# Allow globs that don't return anything
shopt -s nullglob
# Allow globs with ignores
shopt -s extglob

rm -r crates/rendering/shaders/compiled/*.spv

for file in crates/rendering/shaders/*.{vert,frag,comp}
do
output=crates/rendering/shaders/compiled/$(basename $file).spv
glslc $file -o $output
done

for file in crates/rendering/shaders/compiled/*.spv
do
spirv-opt $file -O -o $file
done
