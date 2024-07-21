#!/usr/bin/env python3

'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import hashlib
import itertools
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

class File:
    """A file in the filesystem."""
    def __init__(self, filepath):
        self.path = os.path.normpath(filepath)
        with open(self.path, "rb") as f:
            self.hash = hashlib.md5(f.read()).digest()
        filename = os.path.basename(filepath)
        self.name, self.extension = os.path.splitext(filename)
        self.isimage = self.extension in (".jpg", ".png")
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
    phashes = dict()
    for file in files:
        phashes.setdefault(file.phash, set()).add(file)
    return phashes

def group_similars(phashes):
    similar_groups = set()
    for similars in phashes.values():
        if 1 < len(similars):
            similar_groups.add(frozenset(similars))
    return similar_groups

def make_pairs(similar_groups):
    pairs = list()
    for similars in similar_groups:
        pairs.extend(itertools.combinations(similars, 2))
    return pairs

def make_thumbnail_path(file):
    return os.path.join(TMP_DIR, file.hash.hex() + file.extension)

def make_thumbnails(file):
    subprocess.run(["magick",
                    file.path,
                    "-resize", "100x100",
                    make_thumbnail_path(file)])

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

def ncc_compare(files):
    file1, file2 = files
    thumb1 = make_thumbnail_path(file1)
    thumb2 = make_thumbnail_path(file2)
    return (file1, file2, ncc_score(thumb1, thumb2))

def main(argv):
    print("Looking for files... ", end="", flush=True)
    filepaths = list_files(".")
    len_files = len(filepaths)
    print("found", len_files)
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
    with multiprocessing.Pool() as pool:
        print("  Making thumbnails...")
        list(tqdm.tqdm(pool.imap_unordered(make_thumbnails, tothumb),
                       total=len(tothumb),
                       unit="thumbnails",
                       dynamic_ncols=True))
        print("  Comparing pairs...")
        results = list(tqdm.tqdm(pool.imap_unordered(ncc_compare, pairs),
                                 total=len(pairs),
                                 unit="pairs",
                                 dynamic_ncols=True))

    print("  Compiling results...")
    similars = dict() # File to set of File. sets are shared among multiple keys
    similars = Clusterer()
    for file1, file2, ncc_score in tqdm.tqdm(results, dynamic_ncols=True):
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

    return 0

if __name__ == "__main__":
    exit(main(sys.argv))
