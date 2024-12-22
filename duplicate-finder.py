#!/usr/bin/env python3

'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import hashlib
import itertools
import json
import multiprocessing
import os
import shlex
import subprocess
import sys
import tempfile

import isearch
import tqdm
from PIL import Image

TMP_DIR="/tmp/duplicate-finder"
CACHE_FILE=os.path.expanduser("~/.cache/duplicate-finder_cache.json")

def compute_hash(path):
    with open(path, "rb") as f:
        return hashlib.md5(f.read()).hexdigest()

class File:
    """A file in the filesystem."""
    def __init__(self, filepath):
        self.path = os.path.normpath(filepath)
        self.hash = compute_hash(self.path)
        filename = os.path.basename(filepath)
        self.name, self.extension = os.path.splitext(filename)
        self.isimage = self.extension in (".jpg", ".png", ".webp")
        if self.isimage:
            self.phash = isearch.phash(self.path)

    def __lt__(self, other):
        return self.path < other.path

    def __str__(self):
        return shlex.quote(self.path)

    def __hash__(self):
        return hash(self.path)

    def __eq__(self, other):
        return self.path == other.path

class Clusterer:
    """A manager for SCCs."""
    def __init__(self):
        self.d = dict()

    def add_pair(self, a, b):
        if a in self.d.keys() and b in self.d.keys():
            sa = self.d[a]
            sb = self.d[b]
            # sa and sb have no intersection
            # add all of sb to sa
            sa.update(sb)
            # everyone in sb is now associated with sa
            for i in sb:
                self.d[i] = sa
        elif a in self.d.keys():
            sa = self.d[a]
            # add b to sa
            sa.add(b)
            # b is now associated with sa
            self.d[b] = sa
        elif b in self.d.keys():
            sb = self.d[b]
            # add a to sb
            sb.add(a)
            # a is now associated with sb
            self.d[a] = sb
        else:
            # new SCC
            s = {a, b}
            self.d[a] = s
            self.d[b] = s

    def add_singular(self, a):
        self.d.setdefault(a, {a})

    def compile(self):
        """Get all the SCCs."""
        return frozenset(frozenset(s) for s in self.d.values())

def list_files(dirpath):
    filepaths = list()
    for root, _, files in os.walk(dirpath):
        for file in files:
            filepath = os.path.join(root,file)
            if not os.path.islink(filepath):
                filepaths.append(filepath)
    return filepaths

def match_hashes(files):
    hashes = dict()
    for file in files:
        hashes.setdefault(file.hash, set()).add(file)
    return hashes

def dedup_hashes(hashes):
    return {
            sorted(duplicates)[0]
            for duplicates in hashes.values()
        }

def match_phashes(files):
    raw_phashes = dict()
    similar_phashes = Clusterer()
    for file in files:
        raw_phashes.setdefault(file.phash, set()).add(file)
        similar_phashes.add_singular(file.phash)

    for phash1, phash2 in itertools.combinations(raw_phashes.keys(), 2):
        if (phash1 ^ phash2).bit_count() < 2:
            similar_phashes.add_pair(phash1, phash2)

    phashes = set()
    for phash_group in similar_phashes.compile():
        files_in_group = set()
        for phash in phash_group:
            files_in_group.update(raw_phashes[phash])
        phashes.add(frozenset(files_in_group))

    return frozenset(phashes)

def group_similars(phashes):
    similar_groups = set()
    for similars in phashes:
        if 1 < len(similars):
            similar_groups.add(frozenset(similars))
    return similar_groups

def make_pairs(similar_groups):
    pairs = list()
    for similars in similar_groups:
        pairs.extend(itertools.combinations(similars, 2))
    return pairs

def make_thumbnail_path(file):
    return os.path.join(TMP_DIR, file.hash + file.extension)

def make_thumbnails(file):
    thumbnail_path = make_thumbnail_path(file)
    if os.path.exists(thumbnail_path):
        return
    subprocess.run(["magick",
                    file.path,
                    "-resize", "100x100",
                    thumbnail_path])

CACHE=dict()
def load_cache():
    global CACHE
    try:
        with open(CACHE_FILE) as f:
            CACHE = json.load(f)
        print(f"Loaded {len(CACHE)} cached entries")
    except Exception: # File not found, JSON parsing error, any fs error...
        CACHE=dict()
        print("Could not load cache")

def get_from_cache(key):
    global CACHE
    return CACHE.get(key)

def set_in_cache(key, value):
    global CACHE
    CACHE[key] = value

def store_cache():
    global CACHE
    with open(CACHE_FILE, 'w') as f:
        json.dump(CACHE, f)
    print(f"Stored {len(CACHE)} cached entries")

def ncc_score(thumb1, thumb2):
    # ncc gives an unusable score when the bit depth of file2 is less than file1
    # TODO: find a better compare command
    with Image.open(thumb1) as i1:
        with Image.open(thumb2) as i2:
            if i1.mode != i2.mode:
                return -1
    compare = subprocess.run(
            ["compare", "-metric", "ncc", thumb1, thumb2, "/dev/null"],
            capture_output=True)
    return float(compare.stderr.decode())

def ncc_cache_key(file1, file2):
    hash1, hash2 = sorted((file1.hash, file2.hash))
    return hash1 + hash2

def ncc_cache_dekey(key):
    return key[:32], key[32:]

def ncc_compare(files):
    file1, file2 = files
    if score := get_from_cache(ncc_cache_key(file1, file2)):
        return (file1, file2, score)
    thumb1 = make_thumbnail_path(file1)
    thumb2 = make_thumbnail_path(file2)
    return (file1, file2, ncc_score(thumb1, thumb2))

def duplicate_finder():
    print("Looking for files... ", end="", flush=True)
    filepaths = list_files(".")
    len_files = len(filepaths)
    print("found", len_files)
    load_cache()
    print()
    print("Extracting info from files...")
    with multiprocessing.Pool() as pool:
        files = list(tqdm.tqdm(pool.imap_unordered(File, filepaths),
                               total=len(filepaths),
                               unit="file",
                               dynamic_ncols=True))

    print()
    print("Matching identical files... ", end="", flush=True)
    hashes = match_hashes(files)
    uniques = dedup_hashes(hashes)
    len_uniques = len(uniques)
    if len_uniques == len_files:
        print("all uniques")
    else:
        print(f"{len_uniques} unique files ({len_uniques - len_files})")

    print()
    images = frozenset(filter(lambda f: f.isimage, uniques))
    len_images = len(images)
    print(len_images, "images")
    print("1st pass: perceptual hashing... ", end="", flush=True)
    phashes = match_phashes(images)
    len_phashes = len(phashes)
    print(len_phashes, "perceptually dissimilar groups")

    print("2nd pass: normalized cross correllation... ", end="", flush=True)
    similar_groups = group_similars(phashes)
    pairs = make_pairs(similar_groups)
    print(len(pairs), "pairs to compare")
    os.makedirs(TMP_DIR, exist_ok=True)
    tothumb = set()
    for group in similar_groups:
        tothumb.update(group)
    print("  Making thumbnails...")
    with multiprocessing.Pool() as pool:
        list(tqdm.tqdm(pool.imap_unordered(make_thumbnails, tothumb),
                       total=len(tothumb),
                       unit="thumbnails",
                       dynamic_ncols=True))
    print("  Comparing pairs...")
    with multiprocessing.Pool() as pool:
        it = iter(tqdm.tqdm(pool.imap_unordered(ncc_compare, pairs),
                            total=len(pairs),
                            unit="pairs",
                            dynamic_ncols=True,
                            smoothing=0))
        results = list()
        while True:
            try:
                results.append(next(it))
            except:
                break

    print("  Compiling results...")
    similars = dict() # File to set of File. sets are shared among multiple keys
    similars = Clusterer()
    for file1, file2, ncc_score in tqdm.tqdm(results, dynamic_ncols=True):
        set_in_cache(ncc_cache_key(file1, file2), ncc_score)
        if 0.9 <= ncc_score:
            similars.add_pair(file1, file2)
    similarity_sets = similars.compile()

    print()
    for identicals in hashes.values():
        if 1 < len(identicals):
            print("identical:", *sorted(identicals))
    print()
    for similarity_set in similarity_sets:
        if 1 < len(similarity_set):
            print("similar:", *sorted(similarity_set))

    store_cache()
    return 0

def clean():
    print("Looking for files... ", end="", flush=True)
    filepaths = list_files(".")
    len_files = len(filepaths)
    print("found", len_files)
    load_cache()
    print()
    print("Extracting info from files...")
    with multiprocessing.Pool() as pool:
        hashes_in_folder = frozenset(tqdm.tqdm(pool.imap_unordered(compute_hash,
                                                                   filepaths),
                                               total=len(filepaths),
                                               unit="file",
                                               dynamic_ncols=True))

    print()
    cache_keys_by_hash = dict()
    for key in CACHE.keys():
        hash1, hash2 = ncc_cache_dekey(key)
        cache_keys_by_hash.setdefault(hash1, set()).add(key)
        cache_keys_by_hash.setdefault(hash2, set()).add(key)
    hashes_in_cache = frozenset(cache_keys_by_hash.keys())
    hashes_in_cache_not_in_folder = hashes_in_cache - hashes_in_folder
    len_not_in_folder = len(hashes_in_cache_not_in_folder)
    keys_to_remove = set()
    for h in hashes_in_cache_not_in_folder:
        keys_to_remove |= cache_keys_by_hash[h]
    len_cache = len(CACHE)
    len_toremove = len(keys_to_remove)
    print(f"{len_not_in_folder} files referenced in cache but not in folder. All in all, {len_toremove} entries ({round(len_toremove / len_cache * 100)}%) could be removed from the cache.")

    answer = input("Remove them? [y/N] ").lower()
    if answer == "y":
        for key in keys_to_remove:
            del CACHE[key]
        store_cache()
    return 0

def main(argv):
    if {"-h", "-help", "--help"} & set(argv):
        print("Usage:")
        print("\tduplicate-finder")
        print("\t\tFind and report duplicate and similar files in the current folder")
        print("\tduplicate-finder --clean")
        print("\t\tRemoves entries in the cache that do not reference a file of the current folder")
        return 0
    if "--clean" in argv:
        return clean()
    return duplicate_finder()

if __name__ == "__main__":
    exit(main(sys.argv))
