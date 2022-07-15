#!/bin/bash

# cargo build --release
echo -n "nothing-$1        " >> flag.txt
./target/release/dwraf_generator 2>>  flag.txt > cpp/dwraf.c
clang++ ./cpp/t1.cpp -o nothing-"$1"
strip nothing-"$1"
rm cpp/dwraf.c
