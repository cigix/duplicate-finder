# Duplicate Finder: Find similar files in a hierarchy

Iterate through the files in a hierarchy and look for similitudes between them.
Currently two tests are implemented:
* hash comparison: will find byte-for-byte identical files
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

## Use

Use through the `duplicate-finder.sh` script, which will manage the virtualenv.
Either add this directory to your `$PATH`, or symlink the script in a place that
is already in it.

## Dependencies

* ISearch
* Pillow
* tqdm
* ImageMagick (install through your distribution)
