#!/usr/bin/env python3
'''Compute the difference between file names and find identical files in a
folder hierarchy.'''

from hashlib import md5
from levenshtein import levenshtein
from math import sqrt
from os import walk as explore

paths = list() # {'path': path, 'hash': md5 hash, 'name': basename}
print('Looking for files...', end='')
count = 0
for path, _, files in explore('.'):
    for file in files:
        paths.append({'path': path + '/' + file, 'name': file})
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
        if levenshtein(f1['name'], f2['name']) <= sqrt(len(f1['name'])):
            print(f1['path'], 'and', f2['path'], 'have similar names')
