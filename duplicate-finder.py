#!/usr/bin/env python3
'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import hashlib
import itertools
import json
import math
import multiprocessing
import os
import os.path
import shlex
import subprocess
import sys
import tempfile

import tqdm

from PIL import Image

from levenshtein import levenshtein

COMPARE_IMAGES = '--fast' not in sys.argv
CACHE_FILE = os.path.expanduser("~/.cache/duplicate-finder_cache.json")

# Step 1: list all files
paths = list()
print("Looking for files...")
for root, _, files in os.walk('.'):
    for file in files:
        if not os.path.islink(os.path.join(root,file)):
            paths.append((root, file))
print(f"{len(paths)} files found")

try:
    with open(CACHE_FILE) as f:
        cache = json.load(f)
    print(f"Loaded {len(cache)} cached entries")
except Exception: # File not found, JSON parsing error, any fs error...
    cache = dict()
    print("Could not load cache")

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
    if COMPARE_IMAGES and extension in ['.jpg', '.png']:
        f, tmppath = tempfile.mkstemp(suffix=extension)
        os.close(f)
        subprocess.run(
                ['magick', filepath, '-resize', '100x100', tmppath])
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

def cache_key(file1, file2):
    hash1, hash2 = sorted([file1[2], file2[2]])
    return hash1.hex() + hash2.hex()

def compute_similarity_score(file1, file2):
    # ncc gives an unusable score when the bit depth of file2 is less than file1
    # TODO: find a better compare command
    with Image.open(file1[3]) as i1:
        with Image.open(file2[3]) as i2:
            if i1.mode != i2.mode:
                return None
    compare = subprocess.run(
            ['compare', '-metric', 'ncc', file1[3], file2[3], '/dev/null'],
            capture_output=True)
    return float(compare.stderr.decode())

def compare_files(files):
    file1, file2 = files
    # Similar name
    similar = levenshtein(file1[1], file2[2]) < math.log(len(file1[1]))
    # Similar images
    if file1[3] is not None and file2[3] is not None:
        key = cache_key(file1, file2)
        if key in cache:
            score = cache[key]
        else:
            score = compute_similarity_score(file1, file2)
        return (file1[0], file2[0], similar, score, key)
    return (file1[0], file2[0], similar, None, None)

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
        print("identical:", *map(shlex.quote, sorted(identicals)))
print()
for path1, path2, similar_name, _, _ in comparisons:
    if similar_name:
        print(f"similar name: {shlex.quote(path1)} {shlex.quote(path2)}")
print()
similars = dict() # path to set of similars. sets are shared among multiple keys
for path1, path2, _, similar_img_score, key in comparisons:
    if similar_img_score:
        cache[key] = similar_img_score
        if 0.9 <= similar_img_score <= 1.1:
            if path1 in similars.keys() and path2 in similars.keys():
                similar_to_path2 = similars[path2]
                similars[path1].update(similar_to_path2)
                for path in similar_to_path2:
                    similars[path] = similars[path1]
            elif path1 in similars.keys():
                similars[path1].add(path2)
                similars[path2] = similars[path1]
            elif path2 in similars.keys():
                similars[path2].add(path1)
                similars[path1] = similars[path2]
            else:
                similars[path1] = {path1, path2}
                similars[path2] = similars[path1]
similarity_sets = {frozenset(s) for s in similars.values()}
for similarity_set in similarity_sets:
    print("similar images:", " ".join(map(shlex.quote, similarity_set)))

with open(CACHE_FILE, 'w') as f:
    json.dump(cache, f)

# Step 5: clean up
for _, _, _, tmppath in files:
    if tmppath is not None:
        os.unlink(tmppath)
