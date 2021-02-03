#!/usr/bin/env python3
'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import hashlib
import itertools
import math
import multiprocessing
import os
import subprocess
import tempfile
import os.path

import tqdm

from PIL import Image

from levenshtein import levenshtein

# Step 1: list all files
paths = list()
print("Looking for files...")
for root, _, files in os.walk('.'):
    for file in files:
        paths.append((root, file))
print(f"{len(paths)} files found")

# Step 2: for each file, extract the name, compute the md5, and create a
# resized, smaller copy if it is an image.
def process_file(path):
    root, file = path
    filepath = os.path.join(root, file)
    with open(filepath, 'rb') as f:
        h = hashlib.md5(f.read()).digest()
    name, extension = os.path.splitext(file)
    if extension in ['.jpg', '.png']:
        f, tmppath = tempfile.mkstemp(suffix=extension)
        os.close(f)
        subprocess.run(
                ['convert', '-resize', '100x100', filepath, tmppath])
        return (filepath, name, h, tmppath)
    return (filepath, name, h, None)

print("\nExtracting info from files...")
with multiprocessing.Pool() as pool:
    files = list(
            tqdm.tqdm(
                pool.imap_unordered(process_file, paths),
                total=len(paths)))

# Step 3: for each pair of files, compare their hashes, names, and resized
# images where applicable
def compare_files(files):
    file1, file2 = files
    # Same hash
    identical = file1[2] == file2[2]
    # Similar name
    similar = levenshtein(file1[1], file2[2]) < math.log(len(file1[1]))
    # Similar images
    if file1[3] is not None and file2[3] is not None:
        if identical:
            return (file1[0], file2[0], identical, similar, True)
        with Image.open(file1[3]) as i1:
            with Image.open(file2[3]) as i2:
                # ncc gives an unusable score when the bit depth of file 2 is
                # less than file 1
                # TODO: find a better compare command
                if i1.mode != i2.mode:
                    return (file1[0], file2[0], identical, similar, False)
        compare = subprocess.run(
                ['compare', '-metric', 'ncc', file1[3], file2[3], '/dev/null'],
                capture_output=True)
        score = float(compare.stderr.decode())
        return (file1[0], file2[0], identical, similar, 0.9 <= score <= 1.1)
    return (file1[0], file2[0], identical, similar, False)

print("\nComparing files...")
with multiprocessing.Pool() as pool:
    comparisons = list(
            tqdm.tqdm(
                pool.imap_unordered(
                    compare_files,
                    itertools.combinations(files, 2)),
                total=int(len(files)*(len(files) - 1)/2)))

# Step 4: present results
print()
for path1, path2, identical, similar_name, similar_img in comparisons:
    if identical:
        print(f"{path1} and {path2} are identical")
        continue
    if similar_name:
        print(f"{path1} and {path2} have similar names")
    if similar_img:
        print(f"{path1} and {path2} have similar contents")

# Step 5: clean up
for _, _, _, tmppath in files:
    if tmppath is not None:
        os.unlink(tmppath)
