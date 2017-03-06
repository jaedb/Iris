from __future__ import unicode_literals

import re
from setuptools import setup, find_packages

def get_version(filename):
    content = open(filename).read()
    metadata = dict(re.findall("__([a-z]+)__ = '([^']+)'", content))
    return metadata['version']

setup(
    name='Mopidy-Iris',
    version=get_version('mopidy_iris/__init__.py'),
    url='https://github.com/jaedb/iris',
    license='Apache License, Version 2.0',
    author='James Barnsley',
    author_email='james@barnsley.nz',
    description='A fully-functional Mopidy web client encompassing Spotify and many other backends',
    packages=find_packages(),
    zip_safe=False,
    include_package_data=True,
    install_requires=[
        'setuptools >= 3.3',
        'pylast >= 1.6.0',
        'spotipy >= 2.3.8',
        'Mopidy >= 2.0',
        'Mopidy-Local-Images >= 1.0',
        'ConfigObj >= 5.0.6'
    ],
    classifiers=[
        'Environment :: No Input/Output (Daemon)',
        'Intended Audience :: End Users/Desktop',
        'License :: OSI Approved :: Apache Software License',
        'Operating System :: OS Independent',
        'Programming Language :: Python :: 2',
        'Topic :: Multimedia :: Sound/Audio :: Players',
    ],
    entry_points={
        'mopidy.ext': [
            'iris = mopidy_iris:Extension',
        ],
    },
)
