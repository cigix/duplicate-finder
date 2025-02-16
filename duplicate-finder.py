#!/usr/bin/env python3

'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import hashlib
import itertools
import json
import multiprocessing
import os
import shlex
import shutil
import subprocess
import sys
import tempfile

import isearch
import tqdm
from PIL import Image

TMP_DIR="/tmp/duplicate-finder"
THUMBNAILS_DIR=os.path.join(TMP_DIR, "thumbnails")
TRASH_DIR=os.path.join(TMP_DIR, "trash")
CACHE_FILE=os.path.expanduser("~/.cache/duplicate-finder_cache.json")
REPORT_FILE=os.path.expanduser("~/.cache/duplicate-finder_report.json")

PHASH_DIFF_BITS=2
PROCESS_COUNT=4

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
        if (phash1 ^ phash2).bit_count() <= PHASH_DIFF_BITS:
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
    return os.path.join(THUMBNAILS_DIR, file.hash + file.extension)

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

def store_cache(start_cache_size=None):
    global CACHE
    with open(CACHE_FILE, 'w') as f:
        json.dump(CACHE, f)
    end_cache_size = cache_size()
    if start_cache_size is None:
        print(f"Stored {end_cache_size} cached entries")
    else:
        diff = end_cache_size - start_cache_size
        print(f"Stored {end_cache_size} cached entries ({diff:+})")

def store_report(identicals, similars):
    with open(REPORT_FILE, 'w') as f:
        json.dump({"identicals": identicals, "similars": similars}, f)
    print(f"Report written at {REPORT_FILE}")

def cache_size():
    return len(CACHE)

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

def duplicate_finder(args):
    global PHASH_DIFF_BITS, PROCESS_COUNT
    if 2 <= len(args):
        PHASH_DIFF_BITS = int(args[0])
        PROCESS_COUNT = int(args[1])
    elif 1 == len(args):
        PHASH_DIFF_BITS = int(args[0])
    print("Looking for files... ", end="", flush=True)
    filepaths = list_files(".")
    len_files = len(filepaths)
    print("found", len_files)
    load_cache()
    start_cache_size = cache_size()
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
    print(len_phashes, "perceptually dissimilar groups",
          f"(<={PHASH_DIFF_BITS} bits difference)")

    print("2nd pass: normalized cross correllation... ", end="", flush=True)
    similar_groups = group_similars(phashes)
    pairs = make_pairs(similar_groups)
    print(len(pairs), "pairs to compare")
    os.makedirs(THUMBNAILS_DIR, exist_ok=True)
    tothumb = set()
    for group in similar_groups:
        tothumb.update(group)
    print("  Making thumbnails...")
    with multiprocessing.Pool(4) as pool:
        list(tqdm.tqdm(pool.imap_unordered(make_thumbnails, tothumb),
                       total=len(tothumb),
                       unit="thumbnails",
                       dynamic_ncols=True))
    results = list()
    if PROCESS_COUNT <= 1:
        print(f"  Comparing pairs... (1 process)")
        it = iter(tqdm.tqdm(map(ncc_compare, pairs),
                            total=len(pairs),
                            unit="pairs",
                            dynamic_ncols=True,
                            smoothing=0))
        while True:
            try:
                results.append(next(it))
            except:
                break
    else:
        print(f"  Comparing pairs... ({PROCESS_COUNT} processes)")
        with multiprocessing.Pool(PROCESS_COUNT) as pool:
            it = iter(tqdm.tqdm(pool.imap_unordered(ncc_compare, pairs),
                                total=len(pairs),
                                unit="pairs",
                                dynamic_ncols=True,
                                smoothing=0))
            while True:
                try:
                    results.append(next(it))
                except:
                    break

    print("  Compiling results...")
    identicals = list()
    for identityset in hashes.values():
        if 1 < len(identityset):
            identicals.append(tuple(map(str, sorted(identityset))))
    similars = Clusterer()
    for file1, file2, ncc_score in tqdm.tqdm(results, dynamic_ncols=True):
        set_in_cache(ncc_cache_key(file1, file2), ncc_score)
        if 0.9 <= ncc_score:
            similars.add_pair(file1, file2)
    similaritysets = list()
    for similarityset in similars.compile():
        if 1 < len(similarityset):
            similaritysets.append(tuple(map(str, sorted(similarityset))))

    print()
    store_cache(start_cache_size)
    store_report(identicals, similaritysets)
    print()
    for identityset in identicals:
        print("identical:", *identityset)
    print()
    for similarityset in similaritysets:
        print("similar:", *sorted(similarityset))
    return 0

def clean():
    print("Looking for files... ", end="", flush=True)
    filepaths = list_files(".")
    len_files = len(filepaths)
    print("found", len_files)
    load_cache()
    start_cache_size = cache_size()
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
    len_toremove = len(keys_to_remove)
    print(f"{len_not_in_folder} files referenced in cache but not in folder. All in all, {len_toremove} entries ({round(len_toremove / start_cache_size * 100)}%) could be removed from the cache.")

    answer = input("Remove them? [y/N] ").lower()
    if answer == "y":
        for key in keys_to_remove:
            del CACHE[key]
        store_cache(start_cache_size)
    return 0

def removefile(file):
    try:
        shutil.move(shlex.split(file)[0], TRASH_DIR)
    except shutil.Error as e:
        print(e.args[0])

def interactive():
    try:
        with open(REPORT_FILE) as f:
            report = json.load(f)
    except FileNotFoundError:
        print("Could not find the report file; did you run duplicate-finder beforehand?")
        return 1

    identicals = report["identicals"]
    similars = report["similars"]

    if os.path.isdir(TRASH_DIR):
        if os.listdir(TRASH_DIR):
            print("Trash is not empty.")
            answer = input("Empty it? [Y/n] ").lower()
            if answer == "" or answer == "y":
                shutil.rmtree(TRASH_DIR)
    os.makedirs(TRASH_DIR, exist_ok=True)

    todelete = list()
    for identicalset in identicals:
        todelete.extend(identicalset[1:])
    if todelete:
        print(f"{len(todelete)} have identical matches.")
        answer = input("Delete them? [Y/n] ").lower()
        if answer == "" or answer == "y":
            if 1000 < len(todelete):
                todelete = iter(tqdm.tqdm(todelete))
            for file in todelete:
                try:
                    removefile(file)
                except FileNotFoundError as e:
                    print(f"{e.strerror}: {e.filename}")
            identicals = list()
        print()

    samesize = set()
    diffsize = set()
    others = list()
    for similarityset in similars:
        if len(similarityset) != 2:
            others.append(tuple(similarityset))
            continue
        f1, f2 = similarityset
        i1 = Image.open(f1)
        i2 = Image.open(f2)
        if i1.size == i2.size:
            s1 = os.path.getsize(f1)
            s2 = os.path.getsize(f2)
            if s1 < s2:
                samesize.add((f1, f2))
            else:
                samesize.add((f2, f1))
            continue
        if i1.height < i2.height and i1.width < i2.width:
            diffsize.add((f1, f2))
            continue
        if i2.height < i1.height and i2.width < i1.width:
            diffsize.add((f2, f1))
            continue
        others.append(tuple(similarityset))

    for f1, f2 in diffsize:
        feh = subprocess.Popen(["feh", f1, f2], stdin=subprocess.DEVNULL)
        print("These pictures are similar but of different size.")
        answer = input("Delete the smaller one? [Y/n] ").lower()
        if answer == "" or answer == "y":
            removefile(f1)
        else:
            print("Ok, keeping it for later")
            others.append((f1, f2))
        feh.terminate()
        feh.wait()
        print()

    print("====================\n")

    for f1, f2 in samesize:
        feh = subprocess.Popen(["feh", f1, f2], stdin=subprocess.DEVNULL)
        print("These pictures are similar and have the same size.")
        answer = input("Delete the heavier one? [Y/n] ").lower()
        if answer == "" or answer == "y":
            removefile(f2)
        else:
            print("Ok, keeping it for later")
            others.append((f1, f2))
        feh.terminate()
        feh.wait()
        print()

    store_report(identicals, others)

    if others:
        print("These files still need attention:")
        for similarityset in others:
            print("-", *similarityset)

    return 0

def main(argv):
    if {"-h", "-help", "--help"} & set(argv):
        print("Usage:")
        print("\tduplicate-finder [diff [processes]]")
        print("\t\tFind and report duplicate and similar files in the current folder")
        print("\t\tdiff: # of bits difference to have similar perceptual hash, default", PHASH_DIFF_BITS)
        print("\t\tprocesses: # of processes to parallelize process, default", PROCESS_COUNT)
        print("\tduplicate-finder --clean")
        print("\t\tRemoves entries in the cache that do not reference a file of the current folder")
        print("\tduplicate-finder -i")
        print("\t\tReview the reported results interactively")
        return 0
    if "--clean" in argv:
        return clean()
    if "-i" in argv:
        return interactive()
    return duplicate_finder(argv[1:])

if __name__ == "__main__":
    exit(main(sys.argv))
