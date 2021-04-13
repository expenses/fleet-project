#!/bin/sh

# Allow globs that don't return anything
shopt -s nullglob
# Allow globs with ignores
shopt -s extglob

rm -r shaders/compiled/*.spv

for file in shaders/*.{vert,frag,comp}
do
output=shaders/compiled/$(basename $file).spv
glslc $file -o $output
done

for file in shaders/compiled/*.spv
do
spirv-opt $file -O -o $file
done
