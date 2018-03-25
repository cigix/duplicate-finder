#!/usr/bin/env python3
'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

import sys
import os
import os.path

from hashlib import md5
from levenshtein import levenshtein
from math import sqrt

# {'path': path, 'hash': md5 hash, 'name': basename without extension}
paths = list()
print('Looking for files...', end='')
count = 0
for path, _, files in os.walk('.'):
    for file in files:
        name, extension = os.path.splitext(file)
        # If no extension was given, or this file's extension was given
        if len(sys.argv) == 1 or extension in sys.argv:
            paths.append({'path': path + '/' + file, 'name': name})
            count += 1
print(count, 'files found')

for i, file in enumerate(paths):
    print('\rComputing hashes...{}/{}'.format(i + 1, count), end='')
    with open(file['path'], 'rb') as f:
        content = f.read()
    file['hash'] = md5(content).digest()

print('\nComparing files...')
for i, f1 in enumerate(paths):
    for f2 in paths[i+1:]:
        if f1['hash'] == f2['hash']:
            print(f1['path'], 'and', f2['path'], 'are identical')
        elif levenshtein(f1['name'], f2['name']) <= sqrt(len(f1['name'])):
            print(f1['path'], 'and', f2['path'], 'have similar names')
