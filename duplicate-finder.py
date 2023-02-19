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
        if not os.path.islink(os.path.join(root,file)):
            paths.append((root, file))
print(f"{len(paths)} files found")

# Step 2: for each file, extract the name, compute the md5, and create a
# resized, smaller copy if it is an image.
def process_file(path):
    root, file = path
    filepath = os.path.join(root, file)
    if filepath[:2] == './':
        filepath = filepath[2:]
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
                total=len(paths),
                unit="file",
                dynamic_ncols=True,
                smoothing=0))

# Step 3: for each pair of files, compare their hashes, names, and resized
# images where applicable
hashes = dict()
def register_hash(file):
    hashes.setdefault(file[2], set()).add(file[0])

print("\nFiltering identical files...")
for file in files:
    register_hash(file)
# keep only one from the pack
unique_files = {
        file
        for file in files
        if file[0] == sorted(hashes[file[2]])[0]
    }
print(f"{len(files)} -> {len(unique_files)}")

def compare_files(files):
    file1, file2 = files
    # Similar name
    similar = levenshtein(file1[1], file2[2]) < math.log(len(file1[1]))
    # Similar images
    if file1[3] is not None and file2[3] is not None:
        with Image.open(file1[3]) as i1:
            with Image.open(file2[3]) as i2:
                # ncc gives an unusable score when the bit depth of file 2 is
                # less than file 1
                # TODO: find a better compare command
                if i1.mode != i2.mode:
                    return (file1[0], file2[0], similar, False)
        compare = subprocess.run(
                ['compare', '-metric', 'ncc', file1[3], file2[3], '/dev/null'],
                capture_output=True)
        score = float(compare.stderr.decode())
        return (file1[0], file2[0], similar, 0.9 <= score <= 1.1)
    return (file1[0], file2[0], similar, False)

print("Comparing unique files...")
try:
    with multiprocessing.Pool() as pool:
        comparisons = list(
                tqdm.tqdm(
                    pool.imap_unordered(
                        compare_files,
                        itertools.combinations(unique_files, 2)),
                    total=int(len(unique_files)*(len(unique_files) - 1)/2),
                    unit="files"))
except KeyboardInterrupt:
    comparisons = list()

# Step 4: present results
print()
for identicals in hashes.values():
    if len(identicals) > 1:
        print("identical:", *map(lambda s: f"'{s}'", sorted(identicals)))
print()
for path1, path2, similar_name, similar_img in comparisons:
    if similar_name:
        print(f"similar name: '{path1}' '{path2}'")
    if similar_img:
        print(f"similar images: '{path1}' '{path2}'")

# Step 5: clean up
for _, _, _, tmppath in files:
    if tmppath is not None:
        os.unlink(tmppath)
