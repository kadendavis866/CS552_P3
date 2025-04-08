# CS 552 Operating Systems Project 3

This is my implementation of a buddy memory allocator in rust using the procedure specified by Knuth.
It follows his paper's specifications and methods with the exception that I took a recursive approach in the alloc method rather than the iterative one he used. This simplified the code but likely does not change performance.

Due to the nature of this project, quite a lot of unsafe rust was used for pointer arithmetic and manipulation. I have checked through all of the unsafe methods carefully and believe them to be safe, but further testing should be done before use in a critical project.

Knuth did not specify a procedure for realloc() in his paper but I have implented it in a way that should be a logical extension of his approach.

If any recommendations or bugs are found, please feel free to submit an issue on GitHub.

Steps to configure, build, run, and test the project.

## Building
To build a dynamic rust library, the build files for the library can be found in ./target/debug/:
```bash
make
```

## Testing

```bash
make check
```

## Clean

```bash
make clean
```

## Install Dependencies

If needed, the rust build system (rustup and cargo) can be installed/updated by running the following command:

```bash
sudo make install-deps
```