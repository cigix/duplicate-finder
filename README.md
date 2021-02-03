# Duplicate Finder: Find similar files in a hierarchy

Iterate through the files in a hierarchy and look for similitudes between them.
Currently three tests are implemented:
* hash comparison: will find byte-for-byte identical files
* name comparison: will find files that have similar names
* image comparison: will find images that have similar contents

## Note on image comparison

The metric used for image comparison does not bode well with images in different
colorspaces (grayscale vs. color, for example). For this reason,
`duplicate-finder` does not compare image files that report different
colorspaces.

However, you could have a file encoded with the wrong (not restrictive enough)
colorspace. `duplicate-finder` will also try to filter out the nonsensical
results from the comparison.

The `find-grayscale` program may help you find images that have a larger
colorspace than needed.

## Dependencies

* ImageMagick (install through your distribution)
* Pillow (install with `pip`)
* tqdm (install with `pip`)
