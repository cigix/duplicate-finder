#!/usr/bin/env python3
'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import os

from hashlib import md5
from levenshtein import levenshtein

LEVENSHTEIN_TOLERANCE = 3

paths = list() # {'path': path, 'hash': md5 hash, 'name': basename}
for path, _, files in os.walk('.'):
    for file in files:
        with open(path + '/' + file, 'rb') as f:
            content = f.read()
        paths.append({'path': path + '/' + file,
                      'hash': md5(content).digest(),
                      'name': file})

for i, f1 in enumerate(paths):
    for f2 in paths[i+1:]:
        if f1['hash'] == f2['hash']:
            print(f1['path'], 'and', f2['path'], 'are identical')
        if levenshtein(f1['name'], f2['name']) <= LEVENSHTEIN_TOLERANCE:
            print(f1['path'], 'and', f2['path'], 'have similar names')
